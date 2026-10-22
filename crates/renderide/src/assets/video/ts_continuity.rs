//! MPEG-TS continuity-counter repair for fragmented streams.
//!
//! YouTube HLS and other fragmented MPEG-TS sources reset the 4-bit continuity
//! counter at fragment boundaries even though no packet was lost. `tsdemux`
//! treats that reset as loss and drops the payload packet (and, on GStreamer
//! 1.26 through 1.28, the following PES start as well). The missing access-unit
//! bytes show up as video corruption after the first fragments, or as a decoder
//! error that produces no picture and no audio.
//!
//! GStreamer 1.30 adds `tsdemux:ignore-continuity-counter` for this case. Older
//! versions need the counters rewritten so each PID stays continuous before the
//! demuxer sees the buffer. Real packet loss still leaves a hole where the lost
//! packet was; the rewrite only stops `tsdemux` from discarding the next packet
//! that did arrive.

use std::collections::HashMap;

/// MPEG-TS packet size accepted by `tsdemux`.
const TS_PACKET_SIZE: usize = 188;

/// MPEG-TS sync byte.
const TS_SYNC_BYTE: u8 = 0x47;

/// Null-packet PID. These packets do not carry a stream and do not advance counters.
const NULL_PID: u16 = 0x1FFF;

/// Low nibble of the continuity counter.
const CONTINUITY_MASK: u8 = 0x0F;

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

/// Per-PID continuity state for one `tsdemux` instance.
#[derive(Debug, Default)]
pub(super) struct MpegTsContinuityState {
    /// Last continuity counter accepted for each payload-carrying PID.
    last_payload_cc: HashMap<u16, u8>,
}

impl MpegTsContinuityState {
    /// Forgets counters after a flush or stream start so the next packet is a baseline.
    pub(super) fn reset(&mut self) {
        self.last_payload_cc.clear();
    }
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

/// Rewrites unexpected continuity counters in `data` and returns how many packets changed.
///
/// `data` may contain one or more 188-byte packets. A short or unsynchronized buffer is
/// left untouched. Adaptation-only packets, null packets, and transport-error packets do
/// not advance the per-PID counter. A repeated counter (a duplicate packet) is preserved
/// so the duplicate payload is not appended a second time.
pub(super) fn repair_mpegts_continuity(
    data: &mut [u8],
    state: &mut MpegTsContinuityState,
) -> usize {
    let Some(mut offset) = ts_packet_offset(data) else {
        return 0;
    };
    let mut rewritten = 0;
    while offset + TS_PACKET_SIZE <= data.len() {
        let next = offset + TS_PACKET_SIZE;
        if data[offset] != TS_SYNC_BYTE {
            break;
        }
        if next < data.len() && data.get(next) != Some(&TS_SYNC_BYTE) {
            break;
        }
        if repair_packet(&mut data[offset..next], state) {
            rewritten += 1;
        }
        offset = next;
    }
    rewritten
}

/// Returns the first offset where a 188-byte packet stream is synchronized.
fn ts_packet_offset(data: &[u8]) -> Option<usize> {
    let last_start = data.len().checked_sub(TS_PACKET_SIZE)?;
    let search_end = last_start.min(TS_PACKET_SIZE);
    (0..=search_end).find(|offset| packet_stream_starts_at(data, *offset))
}

/// Returns whether `offset` begins a packet and the following packet is synchronized when present.
fn packet_stream_starts_at(data: &[u8], offset: usize) -> bool {
    if data.get(offset) != Some(&TS_SYNC_BYTE) {
        return false;
    }
    let next = offset + TS_PACKET_SIZE;
    next == data.len() || data.get(next) == Some(&TS_SYNC_BYTE)
}

/// Repairs one 188-byte packet. Returns `true` when the counter byte changed.
fn repair_packet(packet: &mut [u8], state: &mut MpegTsContinuityState) -> bool {
    let Some(header) = TsPacketHeader::parse(packet) else {
        return false;
    };
    if header.transport_error || header.pid == NULL_PID || !header.has_payload {
        return false;
    }
    let expected = match state.last_payload_cc.get(&header.pid).copied() {
        None => {
            state.last_payload_cc.insert(header.pid, header.continuity);
            return false;
        }
        Some(last) => (last + 1) & CONTINUITY_MASK,
    };
    if header.continuity == expected {
        state.last_payload_cc.insert(header.pid, header.continuity);
        return false;
    }
    // A repeated counter is a duplicate packet. Leave it so the demuxer can drop the copy.
    let duplicate = (expected.wrapping_sub(1)) & CONTINUITY_MASK;
    if header.continuity == duplicate {
        return false;
    }
    packet[3] = (packet[3] & !CONTINUITY_MASK) | expected;
    state.last_payload_cc.insert(header.pid, expected);
    true
}

/// Fields of an MPEG-TS packet header that the continuity repair needs.
struct TsPacketHeader {
    /// Packet identifier.
    pid: u16,
    /// Transport error indicator.
    transport_error: bool,
    /// Adaptation field control includes a payload.
    has_payload: bool,
    /// 4-bit continuity counter.
    continuity: u8,
}

impl TsPacketHeader {
    /// Parses a 188-byte packet header.
    fn parse(packet: &[u8]) -> Option<Self> {
        if packet.len() != TS_PACKET_SIZE || packet[0] != TS_SYNC_BYTE {
            return None;
        }
        let adaptation = (packet[3] >> 4) & 0x03;
        Some(Self {
            pid: (u16::from(packet[1] & 0x1F) << 8) | u16::from(packet[2]),
            transport_error: packet[1] & 0x80 != 0,
            has_payload: adaptation == 0b01 || adaptation == 0b11,
            continuity: packet[3] & CONTINUITY_MASK,
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

    fn continuity_of(packet: &[u8]) -> u8 {
        packet[3] & CONTINUITY_MASK
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
    fn continuous_counters_and_payload_bytes_stay_put() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 3, 0x11));
        data.extend(packet(VIDEO_PID, 0b01, 4, 0x22));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 0);
        assert_eq!(continuity_of(&data[..TS_PACKET_SIZE]), 3);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 4);
        assert_eq!(data[4], 0x11);
        assert_eq!(data[TS_PACKET_SIZE + 10], 0x22);
    }

    #[test]
    fn fragment_counter_reset_is_rewritten_without_touching_payload() {
        let mut state = MpegTsContinuityState::default();
        let mut first = packet(VIDEO_PID, 0b01, 5, 0xAB).to_vec();
        assert_eq!(repair_mpegts_continuity(&mut first, &mut state), 0);

        let mut reset = packet(VIDEO_PID, 0b01, 0, 0xCD);
        reset[3] |= 0b1100_0000;
        let payload = reset[4..].to_vec();
        let mut data = reset.to_vec();

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 1);
        assert_eq!(continuity_of(&data), 6);
        assert_eq!(data[3] & 0xF0, 0b1101_0000);
        assert_eq!(&data[4..], payload.as_slice());
    }

    #[test]
    fn fragment_sequence_stays_continuous_after_a_reset() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 5, 1));
        data.extend(packet(VIDEO_PID, 0b01, 0, 2));
        data.extend(packet(VIDEO_PID, 0b01, 1, 3));
        data.extend(packet(VIDEO_PID, 0b01, 2, 4));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 3);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 6);
        assert_eq!(continuity_of(&data[2 * TS_PACKET_SIZE..]), 7);
        assert_eq!(continuity_of(&data[3 * TS_PACKET_SIZE..]), 8);
        assert_eq!(data[TS_PACKET_SIZE + 4], 2);
        assert_eq!(data[3 * TS_PACKET_SIZE + 4], 4);
    }

    #[test]
    fn real_gap_keeps_the_arrived_packet_on_the_next_counter() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 1));
        data.extend(packet(VIDEO_PID, 0b01, 6, 9));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 1);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 5);
        assert_eq!(data[TS_PACKET_SIZE + 4], 9);
    }

    #[test]
    fn counter_wrap_from_15_to_0_is_continuous() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 15, 1));
        data.extend(packet(VIDEO_PID, 0b01, 0, 2));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 0);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 0);
    }

    #[test]
    fn pids_are_tracked_independently() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 1, 1));
        data.extend(packet(AUDIO_PID, 0b01, 9, 2));
        data.extend(packet(VIDEO_PID, 0b01, 0, 3));
        data.extend(packet(AUDIO_PID, 0b01, 10, 4));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 1);
        assert_eq!(continuity_of(&data[2 * TS_PACKET_SIZE..]), 2);
        assert_eq!(
            continuity_of(&data[3 * TS_PACKET_SIZE..]),
            10,
            "audio counter was already continuous"
        );
    }

    #[test]
    fn adaptation_only_packets_do_not_advance_the_counter() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 4, 1));
        data.extend(packet(VIDEO_PID, 0b10, 4, 2));
        data.extend(packet(VIDEO_PID, 0b11, 5, 3));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 0);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 4);
        assert_eq!(continuity_of(&data[2 * TS_PACKET_SIZE..]), 5);
    }

    #[test]
    fn duplicate_counter_is_left_unchanged() {
        let mut state = MpegTsContinuityState::default();
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 2, 1));
        data.extend(packet(VIDEO_PID, 0b01, 2, 9));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 0);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 2);
        assert_eq!(data[TS_PACKET_SIZE + 4], 9);
    }

    #[test]
    fn null_and_transport_error_packets_are_ignored() {
        let mut state = MpegTsContinuityState::default();
        let mut error = packet(VIDEO_PID, 0b01, 1, 7);
        error[1] |= 0x80;
        let mut data = Vec::new();
        data.extend(packet(VIDEO_PID, 0b01, 1, 1));
        data.extend(error);
        data.extend(packet(NULL_PID, 0b01, 0, 8));
        data.extend(packet(VIDEO_PID, 0b01, 2, 3));

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 0);
        assert_eq!(continuity_of(&data[TS_PACKET_SIZE..]), 1);
        assert_eq!(continuity_of(&data[2 * TS_PACKET_SIZE..]), 0);
        assert_eq!(continuity_of(&data[3 * TS_PACKET_SIZE..]), 2);
    }

    #[test]
    fn unsynchronized_and_short_buffers_are_unchanged() {
        let mut state = MpegTsContinuityState::default();
        let mut short = vec![TS_SYNC_BYTE, 0, 1, 2, 3];
        assert_eq!(repair_mpegts_continuity(&mut short, &mut state), 0);

        let mut garbage = vec![0u8; TS_PACKET_SIZE];
        garbage[3] = 0x21;
        assert_eq!(repair_mpegts_continuity(&mut garbage, &mut state), 0);
        assert_eq!(garbage[3], 0x21);
    }

    #[test]
    fn prefixed_packet_stream_is_repaired_from_the_sync_byte() {
        let mut state = MpegTsContinuityState::default();
        let mut data = vec![0xAA, 0xBB, 0xCC, 0xDD];
        data.extend(packet(VIDEO_PID, 0b01, 1, 4));
        data.extend(packet(VIDEO_PID, 0b01, 0, 5));
        let prefix = data[..4].to_vec();

        assert_eq!(repair_mpegts_continuity(&mut data, &mut state), 1);
        assert_eq!(&data[..4], prefix.as_slice());
        assert_eq!(continuity_of(&data[4 + TS_PACKET_SIZE..]), 2);
    }

    #[test]
    fn reset_accepts_the_next_counter_as_a_new_baseline() {
        let mut state = MpegTsContinuityState::default();
        let mut first = packet(VIDEO_PID, 0b01, 4, 1).to_vec();
        assert_eq!(repair_mpegts_continuity(&mut first, &mut state), 0);
        state.reset();

        let mut second = packet(VIDEO_PID, 0b01, 0, 2).to_vec();
        assert_eq!(repair_mpegts_continuity(&mut second, &mut state), 0);
        assert_eq!(continuity_of(&second), 0);
    }
}
