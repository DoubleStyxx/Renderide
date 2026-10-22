//! MPEG-TS continuity-counter repair for fragmented HTTP streams.
//!
//! YouTube HLS and other fragmented MPEG-TS sources reset the 4-bit continuity
//! counter at fragment boundaries even though no packet was lost. `tsdemux`
//! treats that reset as loss and drops the payload packet. The missing access
//! unit bytes show up as video corruption after the first fragments, or as a
//! decoder error that produces no picture and no audio.
//!
//! Upstream buffers are not aligned to 188-byte packets. A remainder is kept in
//! [`MpegTsContinuityState`] until the next buffer completes it, and sync is
//! searched only while the stream is unlocked. That stops a `0x47` inside a
//! payload from being treated as a header when the next buffer arrives.
//!
//! Each PID stores the last original counter and an offset. The counter written
//! back is `original + offset` (mod 16). The offset changes only when the
//! original counter is discontinuous. A repeated original counter with the same
//! payload is a duplicate retransmission, so the output counter repeats and the
//! offset stays put. A fragment that restarts on the same counter value but
//! carries a different payload is a discontinuity, not a duplicate. Two
//! different payloads that share a hash are indistinguishable from a duplicate.

/// MPEG-TS packet size accepted by `tsdemux`.
const TS_PACKET_SIZE: usize = 188;

/// MPEG-TS sync byte.
const TS_SYNC_BYTE: u8 = 0x47;

/// Null-packet PID. These packets do not carry a stream.
const NULL_PID: u16 = 0x1FFF;

/// 13-bit PID space.
const PID_COUNT: usize = 8192;

/// Low nibble of the continuity counter.
const CONTINUITY_MASK: u8 = 0x0F;

/// Adaptation-field flag. This is not the scrambling control in byte 3.
const DISCONTINUITY_INDICATOR: u8 = 0x80;

/// Bytes required to confirm a packet boundary (two sync bytes, one packet apart).
const LOCK_BYTES: usize = TS_PACKET_SIZE * 2;

/// Whether the installed `tsdemux` exposes `ignore-continuity-counter`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IgnoreContinuityProperty {
    /// The property exists and can be enabled.
    Available,
    /// This GStreamer build predates the property.
    Missing,
}

/// How a `tsdemux` element should tolerate fragmented MPEG-TS continuity resets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MpegTsContinuityStrategy {
    /// Set `ignore-continuity-counter` on GStreamer 1.30 and newer.
    IgnoreProperty,
    /// Rewrite counters on the sink pad for older GStreamer.
    RewritePackets,
}

/// What to do with the buffer that was just scanned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ChunkForward {
    /// The input is already a whole number of packets and none of them change.
    Unchanged,
    /// Overwrite these `(offset, byte)` pairs. The buffer length stays the same.
    Patch(Vec<(usize, u8)>),
    /// Forward these bytes instead. Empty means the chunk is held until a later buffer.
    Replace(Vec<u8>),
}

/// Result of pushing one upstream chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChunkRepair {
    /// Bytes that should be forwarded downstream.
    pub forward: ChunkForward,
    /// Packets whose continuity nibble changed.
    pub rewritten: usize,
}

/// Last accepted packet for one PID.
#[derive(Clone, Copy, Debug)]
struct PidContinuity {
    /// Continuity counter as it arrived, before rewriting.
    last_original: u8,
    /// Added to the original counter, mod 16, to produce the output counter.
    offset: u8,
    /// Payload hash used to tell a retransmission from a fragment that reused the counter.
    payload_hash: u64,
}

/// Per-PID continuity state for one `tsdemux` instance.
pub(super) struct MpegTsContinuityState {
    /// Fixed 13-bit PID table. Heap allocated so the 8k entries are not a stack array.
    pids: Box<[Option<PidContinuity>]>,
    /// Incomplete packet bytes that have not been forwarded yet.
    pending: Vec<u8>,
    /// Set once two sync bytes one packet apart have been seen.
    locked: bool,
}

impl Default for MpegTsContinuityState {
    fn default() -> Self {
        Self::new()
    }
}

impl MpegTsContinuityState {
    /// Creates an empty tracker.
    pub(super) fn new() -> Self {
        Self {
            pids: vec![None; PID_COUNT].into_boxed_slice(),
            pending: Vec::new(),
            locked: false,
        }
    }

    /// Forgets counters and any held partial packet after a flush or stream start.
    pub(super) fn reset(&mut self) {
        self.pids.fill(None);
        self.pending.clear();
        self.locked = false;
    }

    /// Scans `chunk` without writing it.
    ///
    /// A trailing partial packet stays in this state and is not part of the
    /// returned bytes. The next call prepends it, so counters stay continuous
    /// across buffers that are not 188-byte aligned.
    pub(super) fn push_chunk(&mut self, chunk: &[u8]) -> ChunkRepair {
        if let Some(repair) = self.try_push_locked(chunk) {
            return repair;
        }
        self.push_assembled(chunk)
    }

    /// Fast path for a locked stream whose pending tail is empty.
    ///
    /// Returns `None` when a packet does not start on the locked grid so the
    /// caller can resync without having updated any PID.
    fn try_push_locked(&mut self, chunk: &[u8]) -> Option<ChunkRepair> {
        if !self.locked || !self.pending.is_empty() {
            return None;
        }
        let complete = chunk.len() - (chunk.len() % TS_PACKET_SIZE);
        let aligned = &chunk[..complete];
        if aligned
            .chunks(TS_PACKET_SIZE)
            .any(|packet| packet.first().copied() != Some(TS_SYNC_BYTE))
        {
            return None;
        }
        let mut patches = Vec::new();
        let mut rewritten = 0;
        for (index, packet) in aligned.chunks(TS_PACKET_SIZE).enumerate() {
            if let Some(byte) = next_continuity_byte(packet, &mut self.pids) {
                patches.push((index * TS_PACKET_SIZE + 3, byte));
                rewritten += 1;
            }
        }
        let forward = if complete == chunk.len() && patches.is_empty() {
            ChunkForward::Unchanged
        } else if complete == chunk.len() {
            ChunkForward::Patch(patches)
        } else {
            let mut output = aligned.to_vec();
            for (offset, byte) in patches {
                if let Some(slot) = output.get_mut(offset) {
                    *slot = byte;
                }
            }
            self.pending.extend_from_slice(&chunk[complete..]);
            ChunkForward::Replace(output)
        };
        Some(ChunkRepair { forward, rewritten })
    }

    /// Copies `chunk` onto any held remainder and emits only whole packets.
    fn push_assembled(&mut self, chunk: &[u8]) -> ChunkRepair {
        let mut data = Vec::with_capacity(self.pending.len() + chunk.len());
        data.append(&mut self.pending);
        data.extend_from_slice(chunk);
        let mut output = Vec::new();
        let mut rewritten = 0;
        loop {
            if !self.locked {
                let Some(start) = find_packet_lock(&data) else {
                    self.keep_unlocked_tail(&mut data);
                    return ChunkRepair {
                        forward: ChunkForward::Replace(output),
                        rewritten,
                    };
                };
                data.drain(..start);
                self.locked = true;
            }
            let (consumed, lost, packet_rewrites) = take_locked_packets(&mut data, &mut self.pids);
            rewritten += packet_rewrites;
            output.extend(consumed);
            if lost {
                self.locked = false;
                continue;
            }
            self.pending = data;
            return ChunkRepair {
                forward: ChunkForward::Replace(output),
                rewritten,
            };
        }
    }

    /// Keeps the tail that might still start a two-packet lock.
    fn keep_unlocked_tail(&mut self, data: &mut Vec<u8>) {
        let keep = LOCK_BYTES.saturating_sub(1).min(data.len());
        let start = data.len() - keep;
        data.drain(..start);
        self.pending = std::mem::take(data);
    }
}

/// Returns whether `uri` is an HTTP(S) source whose fragmented MPEG-TS should be repaired.
///
/// Local files and UDP keep real continuity gaps visible to `tsdemux`.
pub(super) fn repairs_http_source(uri: &str) -> bool {
    let Some((scheme, _)) = uri.split_once("://") else {
        return false;
    };
    scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
}

/// Chooses the continuity fix for an element factory, if it is `tsdemux`.
pub(super) fn continuity_strategy(
    factory_name: &str,
    property: IgnoreContinuityProperty,
) -> Option<MpegTsContinuityStrategy> {
    if factory_name != "tsdemux" {
        return None;
    }
    Some(match property {
        IgnoreContinuityProperty::Available => MpegTsContinuityStrategy::IgnoreProperty,
        IgnoreContinuityProperty::Missing => MpegTsContinuityStrategy::RewritePackets,
    })
}

/// Walks locked packets at the front of `data` and returns them.
///
/// The bool reports that the grid was lost at `data`'s new front.
fn take_locked_packets(
    data: &mut Vec<u8>,
    pids: &mut [Option<PidContinuity>],
) -> (Vec<u8>, bool, usize) {
    let mut cursor = 0;
    let mut rewritten = 0;
    while cursor + TS_PACKET_SIZE <= data.len() {
        if data[cursor] != TS_SYNC_BYTE {
            let consumed = data.drain(..cursor).collect();
            return (consumed, true, rewritten);
        }
        let end = cursor + TS_PACKET_SIZE;
        if let Some(byte) = next_continuity_byte(&data[cursor..end], pids) {
            data[cursor + 3] = byte;
            rewritten += 1;
        }
        cursor = end;
    }
    let consumed = data.drain(..cursor).collect();
    (consumed, false, rewritten)
}

/// Returns the first offset where two sync bytes sit one packet apart.
fn find_packet_lock(data: &[u8]) -> Option<usize> {
    if data.len() < LOCK_BYTES {
        return None;
    }
    let last = data.len() - LOCK_BYTES;
    (0..=last).find(|&offset| {
        data.get(offset) == Some(&TS_SYNC_BYTE)
            && data.get(offset + TS_PACKET_SIZE) == Some(&TS_SYNC_BYTE)
    })
}

/// Returns a replacement for byte 3 when this packet's counter must change.
fn next_continuity_byte(packet: &[u8], pids: &mut [Option<PidContinuity>]) -> Option<u8> {
    let header = TsPacketHeader::parse(packet)?;
    if header.discontinuity {
        clear_pid(pids, header.pid);
        return None;
    }
    if header.transport_error || header.pid == NULL_PID || !header.has_payload {
        return None;
    }
    let hash = payload_hash(packet, header.payload_offset);
    let slot = pids.get_mut(usize::from(header.pid))?;
    let Some(previous) = *slot else {
        *slot = Some(PidContinuity {
            last_original: header.continuity,
            offset: 0,
            payload_hash: hash,
        });
        return None;
    };
    if header.continuity == previous.last_original && hash == previous.payload_hash {
        // Duplicate retransmission. Leave the offset alone so the output counter repeats.
        return replacement_byte(
            packet[3],
            output_counter(header.continuity, previous.offset),
        );
    }
    let offset = if header.continuity == next_counter(previous.last_original) {
        previous.offset
    } else {
        let expected_output = next_counter(output_counter(previous.last_original, previous.offset));
        expected_output.wrapping_sub(header.continuity) & CONTINUITY_MASK
    };
    *slot = Some(PidContinuity {
        last_original: header.continuity,
        offset,
        payload_hash: hash,
    });
    replacement_byte(packet[3], output_counter(header.continuity, offset))
}

/// Clears one PID. A discontinuity indicator starts a new baseline on the next payload.
fn clear_pid(pids: &mut [Option<PidContinuity>], pid: u16) {
    if let Some(slot) = pids.get_mut(usize::from(pid)) {
        *slot = None;
    }
}

/// Continuity counter written into the packet.
fn output_counter(original: u8, offset: u8) -> u8 {
    original.wrapping_add(offset) & CONTINUITY_MASK
}

/// Next 4-bit counter.
fn next_counter(counter: u8) -> u8 {
    counter.wrapping_add(1) & CONTINUITY_MASK
}

/// Byte 3 with the continuity nibble replaced, or `None` when it already matches.
fn replacement_byte(byte3: u8, counter: u8) -> Option<u8> {
    let rewritten = (byte3 & !CONTINUITY_MASK) | (counter & CONTINUITY_MASK);
    (rewritten != byte3).then_some(rewritten)
}

/// FNV-1a over the packet payload.
fn payload_hash(packet: &[u8], start: usize) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let payload = packet.get(start..).unwrap_or_default();
    for &byte in payload {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Fields of an MPEG-TS packet header that the continuity repair needs.
struct TsPacketHeader {
    /// Packet identifier.
    pid: u16,
    /// Transport error indicator.
    transport_error: bool,
    /// Adaptation field sets `discontinuity_indicator`.
    discontinuity: bool,
    /// Adaptation field control includes a payload.
    has_payload: bool,
    /// 4-bit continuity counter.
    continuity: u8,
    /// First payload byte.
    payload_offset: usize,
}

impl TsPacketHeader {
    /// Parses a 188-byte packet header.
    fn parse(packet: &[u8]) -> Option<Self> {
        if packet.len() != TS_PACKET_SIZE || packet.first().copied() != Some(TS_SYNC_BYTE) {
            return None;
        }
        let adaptation = (packet[3] >> 4) & 0x03;
        let has_adaptation = adaptation == 0b10 || adaptation == 0b11;
        let has_payload = adaptation == 0b01 || adaptation == 0b11;
        let payload_offset = if has_adaptation {
            let length = usize::from(packet.get(4).copied().unwrap_or(0));
            (5 + length).min(packet.len())
        } else {
            4
        };
        let discontinuity = has_adaptation
            && packet.get(4).copied().unwrap_or(0) > 0
            && packet
                .get(5)
                .is_some_and(|flags| flags & DISCONTINUITY_INDICATOR != 0);
        Some(Self {
            pid: (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]),
            transport_error: packet[1] & 0x80 != 0,
            discontinuity,
            has_payload,
            continuity: packet[3] & CONTINUITY_MASK,
            payload_offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIDEO_PID: u16 = 0x0100;
    const AUDIO_PID: u16 = 0x0101;

    fn packet(pid: u16, adaptation: u8, continuity: u8, payload: u8) -> [u8; TS_PACKET_SIZE] {
        let mut bytes = [payload; TS_PACKET_SIZE];
        bytes[0] = TS_SYNC_BYTE;
        bytes[1] = u8::try_from((pid >> 8) & 0x1F).unwrap_or(0);
        bytes[2] = u8::try_from(pid & 0xFF).unwrap_or(0);
        bytes[3] = ((adaptation & 0x03) << 4) | (continuity & CONTINUITY_MASK);
        bytes
    }

    fn with_discontinuity(
        mut packet: [u8; TS_PACKET_SIZE],
        continuity: u8,
    ) -> [u8; TS_PACKET_SIZE] {
        packet[3] = (0b11 << 4) | (continuity & CONTINUITY_MASK);
        packet[4] = 1;
        packet[5] = DISCONTINUITY_INDICATOR;
        packet
    }

    fn continuity_of(packet: &[u8]) -> u8 {
        packet[3] & CONTINUITY_MASK
    }

    fn materialize(input: &[u8], forward: ChunkForward) -> Vec<u8> {
        match forward {
            ChunkForward::Unchanged => input.to_vec(),
            ChunkForward::Patch(patches) => {
                let mut output = input.to_vec();
                for (index, byte) in patches {
                    output[index] = byte;
                }
                output
            }
            ChunkForward::Replace(bytes) => bytes,
        }
    }

    fn push_all(state: &mut MpegTsContinuityState, data: &[u8]) -> (Vec<u8>, usize) {
        let repair = state.push_chunk(data);
        (materialize(data, repair.forward), repair.rewritten)
    }

    fn push_chunks(data: &[u8], chunk_size: usize) -> Vec<u8> {
        let mut state = MpegTsContinuityState::new();
        let mut output = Vec::new();
        for chunk in data.chunks(chunk_size.max(1)) {
            let repair = state.push_chunk(chunk);
            output.extend(materialize(chunk, repair.forward));
        }
        output
    }

    #[test]
    fn strategy_is_only_for_tsdemux() {
        assert_eq!(
            continuity_strategy("h264parse", IgnoreContinuityProperty::Missing),
            None
        );
        assert_eq!(
            continuity_strategy("tsdemux", IgnoreContinuityProperty::Available),
            Some(MpegTsContinuityStrategy::IgnoreProperty)
        );
        assert_eq!(
            continuity_strategy("tsdemux", IgnoreContinuityProperty::Missing),
            Some(MpegTsContinuityStrategy::RewritePackets)
        );
    }

    #[test]
    fn http_sources_are_repaired_and_file_or_udp_are_not() {
        assert!(repairs_http_source("https://example.com/playlist.m3u8"));
        assert!(repairs_http_source("http://example.com/stream.ts"));
        assert!(repairs_http_source("HTTPS://cdn.example/seg.ts"));
        assert!(!repairs_http_source("file:///tmp/video.ts"));
        assert!(!repairs_http_source("udp://239.0.0.1:1234"));
        assert!(!repairs_http_source("rtmp://example/live"));
        assert!(!repairs_http_source("/tmp/video.ts"));
    }

    #[test]
    fn continuous_counters_and_payload_bytes_stay_put() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 3, 0x11));
        data.extend(packet(VIDEO_PID, 0b01, 4, 0x22));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[..TS_PACKET_SIZE]), 3);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 4);
        assert_eq!(output[4], 0x11);
        assert_eq!(output[TS_PACKET_SIZE + 10], 0x22);
    }

    #[test]
    fn fragment_counter_reset_keeps_scrambling_bits_and_payload() {
        let mut state = MpegTsContinuityState::new();
        let (first, rewritten) = push_all(&mut state, &packet(VIDEO_PID, 0b01, 5, 0xAB));
        assert!(first.is_empty(), "one packet cannot confirm the next sync");
        assert_eq!(rewritten, 0);

        let mut reset = packet(VIDEO_PID, 0b01, 0, 0xCD);
        reset[3] |= 0b1100_0000;
        let payload = reset[4..].to_vec();
        let (output, rewritten) = push_all(&mut state, &reset);

        assert_eq!(rewritten, 1);
        assert_eq!(output.len(), TS_PACKET_SIZE * 2);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 6);
        assert_eq!(output[TS_PACKET_SIZE + 3] & 0xF0, 0b1101_0000);
        assert_eq!(&output[TS_PACKET_SIZE + 4..], payload.as_slice());
    }

    #[test]
    fn fragment_sequence_stays_continuous_after_a_reset() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 5, 1));
        data.extend(packet(VIDEO_PID, 0b01, 0, 2));
        data.extend(packet(VIDEO_PID, 0b01, 1, 3));
        data.extend(packet(VIDEO_PID, 0b01, 2, 4));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 3);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 6);
        assert_eq!(continuity_of(&output[2 * TS_PACKET_SIZE..]), 7);
        assert_eq!(continuity_of(&output[3 * TS_PACKET_SIZE..]), 8);
        assert_eq!(output[TS_PACKET_SIZE + 4], 2);
        assert_eq!(output[3 * TS_PACKET_SIZE + 4], 4);
    }

    #[test]
    fn duplicate_after_a_reset_repeats_the_output_counter() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 5, 1));
        data.extend(packet(VIDEO_PID, 0b01, 0, 2));
        data.extend(packet(VIDEO_PID, 0b01, 1, 3));
        data.extend(packet(VIDEO_PID, 0b01, 2, 4));
        data.extend(packet(VIDEO_PID, 0b01, 2, 4));
        data.extend(packet(VIDEO_PID, 0b01, 3, 5));

        let (output, rewritten) = push_all(&mut state, &data);
        let counters: Vec<u8> = output.chunks(TS_PACKET_SIZE).map(continuity_of).collect();
        assert_eq!(counters, vec![5, 6, 7, 8, 8, 9]);
        assert_eq!(rewritten, 5);
        assert_eq!(output[4 * TS_PACKET_SIZE + 4], 4);
    }

    #[test]
    fn same_counter_with_a_new_payload_is_a_fragment_restart() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 0x11));
        data.extend(packet(VIDEO_PID, 0b01, 5, 0x11));
        data.extend(packet(VIDEO_PID, 0b01, 5, 0x22));

        let (output, rewritten) = push_all(&mut state, &data);
        let counters: Vec<u8> = output.chunks(TS_PACKET_SIZE).map(continuity_of).collect();
        assert_eq!(counters, vec![4, 5, 6]);
        assert_eq!(rewritten, 1);
        assert_eq!(output[2 * TS_PACKET_SIZE + 4], 0x22);
    }

    #[test]
    fn real_gap_keeps_the_arrived_packet_on_the_next_counter() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 1));
        data.extend(packet(VIDEO_PID, 0b01, 6, 9));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 1);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 5);
        assert_eq!(output[TS_PACKET_SIZE + 4], 9);
    }

    #[test]
    fn counter_wrap_from_15_to_0_is_continuous() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 15, 1));
        data.extend(packet(VIDEO_PID, 0b01, 0, 2));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 0);
    }

    #[test]
    fn pids_are_tracked_independently() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 1, 1));
        data.extend(packet(AUDIO_PID, 0b01, 9, 2));
        data.extend(packet(VIDEO_PID, 0b01, 0, 3));
        data.extend(packet(AUDIO_PID, 0b01, 10, 4));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 1);
        assert_eq!(continuity_of(&output[2 * TS_PACKET_SIZE..]), 2);
        assert_eq!(continuity_of(&output[3 * TS_PACKET_SIZE..]), 10);
    }

    #[test]
    fn adaptation_only_packets_do_not_advance_the_counter() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 1));
        data.extend(packet(VIDEO_PID, 0b10, 4, 2));
        data.extend(packet(VIDEO_PID, 0b11, 5, 3));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 4);
        assert_eq!(continuity_of(&output[2 * TS_PACKET_SIZE..]), 5);
    }

    #[test]
    fn discontinuity_indicator_resets_the_baseline_without_rewriting() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 1));
        data.extend(with_discontinuity(packet(VIDEO_PID, 0b01, 9, 2), 9));
        data.extend(packet(VIDEO_PID, 0b01, 1, 3));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 9);
        assert_eq!(output[TS_PACKET_SIZE + 5], DISCONTINUITY_INDICATOR);
        assert_eq!(continuity_of(&output[2 * TS_PACKET_SIZE..]), 1);
    }

    #[test]
    fn duplicate_counter_is_left_unchanged_before_any_offset() {
        let mut state = MpegTsContinuityState::new();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 2, 1));
        data.extend(packet(VIDEO_PID, 0b01, 2, 1));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 2);
        assert_eq!(output[TS_PACKET_SIZE + 4], 1);
    }

    #[test]
    fn null_and_transport_error_packets_are_ignored() {
        let mut state = MpegTsContinuityState::new();
        let mut error = packet(VIDEO_PID, 0b01, 1, 7);
        error[1] |= 0x80;
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 1, 1));
        data.extend(error);
        data.extend(packet(NULL_PID, 0b01, 0, 8));
        data.extend(packet(VIDEO_PID, 0b01, 2, 3));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 1);
        assert_eq!(continuity_of(&output[2 * TS_PACKET_SIZE..]), 0);
        assert_eq!(continuity_of(&output[3 * TS_PACKET_SIZE..]), 2);
    }

    #[test]
    fn unsynchronized_and_short_buffers_are_not_forwarded() {
        let mut state = MpegTsContinuityState::new();
        let short = [TS_SYNC_BYTE, 0, 1, 2, 3];
        let (output, rewritten) = push_all(&mut state, &short);
        assert_eq!(rewritten, 0);
        assert!(output.is_empty());

        let mut garbage = vec![0u8; TS_PACKET_SIZE];
        garbage[3] = 0x21;
        let (output, rewritten) = push_all(&mut state, &garbage);
        assert_eq!(rewritten, 0);
        assert!(output.is_empty());
        assert_eq!(garbage[3], 0x21);
    }

    #[test]
    fn prefixed_packet_stream_drops_the_unsynced_prefix() {
        let mut state = MpegTsContinuityState::new();
        let mut data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        data.extend(packet(VIDEO_PID, 0b01, 1, 4));
        data.extend(packet(VIDEO_PID, 0b01, 0, 5));

        let (output, rewritten) = push_all(&mut state, &data);
        assert_eq!(rewritten, 1);
        assert_eq!(output.len(), TS_PACKET_SIZE * 2);
        assert_eq!(output[0], TS_SYNC_BYTE);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 2);
    }

    #[test]
    fn reset_accepts_the_next_counter_as_a_new_baseline() {
        let mut state = MpegTsContinuityState::new();
        let first = packet(VIDEO_PID, 0b01, 4, 1);
        let second = packet(VIDEO_PID, 0b01, 5, 2);
        let _ = push_all(&mut state, &first);
        let _ = push_all(&mut state, &second);
        state.reset();

        let mut restarted = Vec::new();
        restarted.extend(packet(VIDEO_PID, 0b01, 0, 3));
        restarted.extend(packet(VIDEO_PID, 0b01, 1, 4));
        let (output, rewritten) = push_all(&mut state, &restarted);
        assert_eq!(rewritten, 0);
        assert_eq!(continuity_of(&output), 0);
        assert_eq!(continuity_of(&output[TS_PACKET_SIZE..]), 1);
    }

    #[test]
    fn partial_header_is_rewritten_only_after_the_packet_completes() {
        let mut state = MpegTsContinuityState::new();
        let baseline: Vec<u8> = packet(VIDEO_PID, 0b01, 5, 1)
            .into_iter()
            .chain(packet(VIDEO_PID, 0b01, 6, 2))
            .collect();
        let _ = push_all(&mut state, &baseline);

        let reset = packet(VIDEO_PID, 0b01, 0, 9);
        let (held, rewritten) = push_all(&mut state, &reset[..100]);
        assert!(held.is_empty());
        assert_eq!(rewritten, 0);

        let (output, rewritten) = push_all(&mut state, &reset[100..]);
        assert_eq!(rewritten, 1);
        assert_eq!(output.len(), TS_PACKET_SIZE);
        assert_eq!(continuity_of(&output), 7);
        assert_eq!(output[4], 9);
    }

    #[test]
    fn odd_chunk_sizes_match_and_do_not_rewrite_payload() {
        let mut stream = Vec::new();
        stream.extend(packet(VIDEO_PID, 0b01, 5, 0x47));
        stream.extend(packet(VIDEO_PID, 0b01, 6, 0x47));
        stream.extend(packet(VIDEO_PID, 0b01, 0, 0x47));
        stream.extend(packet(VIDEO_PID, 0b01, 1, 0x47));
        stream.extend(packet(VIDEO_PID, 0b01, 1, 0x47));
        stream.extend(packet(AUDIO_PID, 0b01, 3, 0x47));
        stream.extend(packet(AUDIO_PID, 0b01, 4, 0x47));
        stream.extend(with_discontinuity(packet(VIDEO_PID, 0b01, 4, 0x47), 4));
        stream.extend(packet(VIDEO_PID, 0b01, 0, 0x11));

        let aligned = push_chunks(&stream, stream.len());
        for size in [1, 100, 188, 1000, 4096] {
            let chunked = push_chunks(&stream, size);
            assert_eq!(chunked, aligned, "chunk size {size}");
        }
        for packet in aligned.chunks(TS_PACKET_SIZE).take(5) {
            assert!(packet[4..].iter().all(|byte| *byte == 0x47));
        }
    }

    #[test]
    fn aligned_followup_patches_in_place() {
        let mut state = MpegTsContinuityState::new();
        let mut first = Vec::new();
        first.extend(packet(VIDEO_PID, 0b01, 5, 1));
        first.extend(packet(VIDEO_PID, 0b01, 6, 2));
        let repair = state.push_chunk(&first);
        assert!(matches!(repair.forward, ChunkForward::Replace(_)));
        assert_eq!(repair.rewritten, 0);

        let mut next = Vec::new();
        next.extend(packet(VIDEO_PID, 0b01, 0, 3));
        next.extend(packet(VIDEO_PID, 0b01, 1, 4));
        let repair = state.push_chunk(&next);
        assert_eq!(repair.rewritten, 2);
        assert!(matches!(repair.forward, ChunkForward::Patch(_)));
    }

    #[test]
    fn hls_bitrate_repair_is_faster_than_realtime() {
        const BITS_PER_SECOND: usize = 8_000_000;
        const SECONDS: usize = 10;
        let packets = (BITS_PER_SECOND / 8 * SECONDS) / TS_PACKET_SIZE;
        let mut stream = Vec::with_capacity(packets * TS_PACKET_SIZE);
        for index in 0..packets {
            let continuity = u8::try_from(index % 16).unwrap_or(0);
            let continuity = if index % 7_000 == 0 { 0 } else { continuity };
            stream.extend(packet(VIDEO_PID, 0b01, continuity, 0x5A));
        }

        let started = std::time::Instant::now();
        let chunked = push_chunks(&stream, 1000);
        let unaligned = started.elapsed();
        let started = std::time::Instant::now();
        let aligned = push_chunks(&stream, 7 * TS_PACKET_SIZE);
        let aligned_elapsed = started.elapsed();

        assert_eq!(chunked.len(), stream.len());
        assert_eq!(aligned, chunked);
        eprintln!(
            "mpeg-ts continuity repair, test profile: {:.3} ms for {SECONDS}s at 8 Mbps in 1000-byte chunks ({:.0}x realtime), {:.3} ms aligned",
            unaligned.as_secs_f64() * 1_000.0,
            SECONDS as f64 / unaligned.as_secs_f64(),
            aligned_elapsed.as_secs_f64() * 1_000.0
        );
        assert!(
            unaligned.as_secs_f64() < (SECONDS as f64) / 5.0,
            "unaligned repair was slower than 5x realtime"
        );
    }
}
