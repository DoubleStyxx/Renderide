//! Attaches the MPEG-TS continuity fix to playbin's `tsdemux` elements.
//!
//! See [`super::ts_continuity`] for why fragmented HTTP streams otherwise
//! corrupt or stall decoded video and audio. Local files and UDP are left
//! alone so a real continuity gap is still visible to the demuxer.

use std::sync::{Arc, Mutex};

use gstreamer::prelude::{ElementExt, GstObjectExt, ObjectExt, PadExtManual};

use super::ts_continuity::{
    ChunkForward, IgnoreContinuityProperty, MpegTsContinuityState, MpegTsContinuityStrategy,
    continuity_strategy, repairs_http_source,
};

/// `tsdemux` property added in GStreamer 1.30.
const IGNORE_CONTINUITY_PROPERTY: &str = "ignore-continuity-counter";

/// Connects playbin `element-setup` so every `tsdemux` tolerates fragment counter resets.
///
/// The handler is owned by `pipeline` and is released with it. `uri` selects HTTP(S)
/// sources; other schemes skip the fix.
pub(super) fn attach_mpegts_continuity_fix(
    pipeline: &gstreamer::Element,
    asset_id: i32,
    uri: &str,
) {
    if !repairs_http_source(uri) {
        logger::debug!(
            "video texture {asset_id}: MPEG-TS continuity repair skipped for non-HTTP source"
        );
        return;
    }
    let _handler = pipeline.connect("element-setup", false, move |values| {
        let element = values
            .get(1)
            .and_then(|value| value.get::<gstreamer::Element>().ok())?;
        configure_added_element(asset_id, &element);
        None
    });
}

/// Applies the continuity strategy to one element added by playbin.
fn configure_added_element(asset_id: i32, element: &gstreamer::Element) {
    let Some(factory) = element.factory() else {
        return;
    };
    let property = if element.find_property(IGNORE_CONTINUITY_PROPERTY).is_some() {
        IgnoreContinuityProperty::Available
    } else {
        IgnoreContinuityProperty::Missing
    };
    let Some(strategy) = continuity_strategy(factory.name().as_str(), property) else {
        return;
    };
    match strategy {
        MpegTsContinuityStrategy::IgnoreProperty => {
            // `ignore-continuity-counter` is a boolean GObject property (default
            // false, mutable while playing, Since: 1.30). `set_property` sends a
            // Rust `bool`, which glib stores as a boolean GValue. `find_property`
            // checks the name only, so this assumes that type stays boolean.
            element.set_property(IGNORE_CONTINUITY_PROPERTY, true);
            logger::info!("video texture {asset_id}: enabled tsdemux ignore-continuity-counter");
        }
        MpegTsContinuityStrategy::RewritePackets => install_rewrite_probe(asset_id, element),
    }
}

/// Rewrites continuity counters on the `tsdemux` sink pad.
fn install_rewrite_probe(asset_id: i32, element: &gstreamer::Element) {
    let Some(pad) = element.static_pad("sink") else {
        logger::warn!(
            "video texture {asset_id}: tsdemux has no sink pad; continuity repair was not installed"
        );
        return;
    };
    let state = Arc::new(Mutex::new(MpegTsContinuityState::new()));
    let probe = pad.add_probe(
        gstreamer::PadProbeType::BUFFER
            | gstreamer::PadProbeType::EVENT_DOWNSTREAM
            | gstreamer::PadProbeType::EVENT_FLUSH,
        move |_pad, info| {
            profiling::scope!("video::mpegts_continuity");
            repair_probe_info(info, &state, asset_id)
        },
    );
    if probe.is_none() {
        logger::warn!("video texture {asset_id}: failed to install tsdemux continuity repair");
        return;
    }
    logger::info!("video texture {asset_id}: installed MPEG-TS continuity repair on tsdemux");
}

/// Resets counters on flush or stream start, and repairs buffer payloads.
fn repair_probe_info(
    info: &mut gstreamer::PadProbeInfo<'_>,
    state: &Mutex<MpegTsContinuityState>,
    asset_id: i32,
) -> gstreamer::PadProbeReturn {
    if let Some(event) = info.event() {
        if event_resets_continuity(event.type_()) {
            with_continuity_state(state, MpegTsContinuityState::reset);
        }
        return gstreamer::PadProbeReturn::Ok;
    }
    let (rewritten, held) = with_continuity_state(state, |state| repair_probe_buffers(info, state));
    if rewritten > 0 {
        logger::debug!("video texture {asset_id}: rewrote {rewritten} MPEG-TS continuity counters");
    }
    // `Handled` means this probe already unref'd the buffer. `Drop` would make
    // GStreamer unref `info->data` after `take_buffer` cleared it.
    if held {
        gstreamer::PadProbeReturn::Handled
    } else {
        gstreamer::PadProbeReturn::Ok
    }
}

/// Returns whether this event starts a new packet sequence.
fn event_resets_continuity(event_type: gstreamer::EventType) -> bool {
    matches!(
        event_type,
        gstreamer::EventType::FlushStart
            | gstreamer::EventType::FlushStop
            | gstreamer::EventType::StreamStart
    )
}

/// Runs `body` with the continuity state, recovering the guard if a probe panicked earlier.
fn with_continuity_state<T>(
    state: &Mutex<MpegTsContinuityState>,
    body: impl FnOnce(&mut MpegTsContinuityState) -> T,
) -> T {
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            logger::warn!(
                "mpeg-ts continuity state lock poisoned; continuing with the recovered counters"
            );
            poisoned.into_inner()
        }
    };
    body(&mut guard)
}

/// Repairs a single buffer or every buffer in a list.
fn repair_probe_buffers(
    info: &mut gstreamer::PadProbeInfo<'_>,
    state: &mut MpegTsContinuityState,
) -> (usize, bool) {
    if let Some(buffer) = info.take_buffer() {
        return place_edited_buffer(info, edit_buffer(buffer, state));
    }
    let Some(list) = info.take_buffer_list() else {
        return (0, false);
    };
    place_edited_list(info, list, state)
}

/// Puts one edited buffer back on the probe, or holds it when the chunk is incomplete.
fn place_edited_buffer(
    info: &mut gstreamer::PadProbeInfo<'_>,
    edited: (EditedBuffer, usize),
) -> (usize, bool) {
    let (edited, rewritten) = edited;
    match edited {
        EditedBuffer::Keep(buffer) | EditedBuffer::Forward(buffer) => {
            info.data = Some(gstreamer::PadProbeData::Buffer(buffer));
            (rewritten, false)
        }
        EditedBuffer::Hold => (rewritten, true),
    }
}

/// Applies the repair to each buffer. An unchanged list is forwarded as it arrived.
fn place_edited_list(
    info: &mut gstreamer::PadProbeInfo<'_>,
    list: gstreamer::BufferList,
    state: &mut MpegTsContinuityState,
) -> (usize, bool) {
    let mut rewritten = 0;
    let mut changed = false;
    let mut rebuilt = gstreamer::BufferList::new_sized(list.len());
    for index in 0..list.len() {
        let Some(buffer) = list.get_owned(index) else {
            continue;
        };
        let (edited, count) = edit_buffer(buffer, state);
        rewritten += count;
        match edited {
            EditedBuffer::Keep(buffer) => rebuilt.make_mut().add(buffer),
            EditedBuffer::Hold => changed = true,
            EditedBuffer::Forward(buffer) => {
                changed = true;
                rebuilt.make_mut().add(buffer);
            }
        }
    }
    if changed && rebuilt.is_empty() {
        return (rewritten, true);
    }
    let forwarded = if changed { rebuilt } else { list };
    info.data = Some(gstreamer::PadProbeData::BufferList(forwarded));
    (rewritten, false)
}

/// A scanned buffer, after the read-only continuity pass.
enum EditedBuffer {
    /// No counter changed and the bytes are still aligned, so the original buffer is reused.
    Keep(gstreamer::Buffer),
    /// Counters were rewritten, or a partial packet was spliced in.
    Forward(gstreamer::Buffer),
    /// The chunk is only a partial packet. It stays in the continuity state.
    Hold,
}

/// Scans `buffer` read-only and copies it only when the forwarded bytes differ.
fn edit_buffer(
    buffer: gstreamer::Buffer,
    state: &mut MpegTsContinuityState,
) -> (EditedBuffer, usize) {
    let Ok(mapped) = buffer.map_readable() else {
        return (EditedBuffer::Keep(buffer), 0);
    };
    let repair = state.push_chunk(mapped.as_slice());
    drop(mapped);
    let rewritten = repair.rewritten;
    let edited = match repair.forward {
        ChunkForward::Unchanged => EditedBuffer::Keep(buffer),
        ChunkForward::Patch(patches) => patch_or_replace(buffer, &patches),
        ChunkForward::Replace(bytes) if bytes.is_empty() => EditedBuffer::Hold,
        ChunkForward::Replace(bytes) => EditedBuffer::Forward(timed_buffer(&buffer, bytes)),
    };
    (edited, rewritten)
}

/// Rewrites counters in place. A new buffer is built only if the original cannot be mapped writable.
fn patch_or_replace(buffer: gstreamer::Buffer, patches: &[(usize, u8)]) -> EditedBuffer {
    let mut buffer = buffer;
    if patch_in_place(&mut buffer, patches) {
        return EditedBuffer::Forward(buffer);
    }
    let Ok(mapped) = buffer.map_readable() else {
        return EditedBuffer::Keep(buffer);
    };
    let mut owned = mapped.as_slice().to_vec();
    drop(mapped);
    apply_patches(&mut owned, patches);
    EditedBuffer::Forward(timed_buffer(&buffer, owned))
}

/// Applies patches with `make_mut`, which copies the buffer only when it is not writable.
fn patch_in_place(buffer: &mut gstreamer::Buffer, patches: &[(usize, u8)]) -> bool {
    let writable = buffer.make_mut();
    let Ok(mut mapped) = writable.map_writable() else {
        return false;
    };
    let slice = mapped.as_mut_slice();
    for &(index, byte) in patches {
        let Some(slot) = slice.get_mut(index) else {
            return false;
        };
        *slot = byte;
    }
    true
}

/// Writes replacement continuity bytes into an owned packet copy.
fn apply_patches(bytes: &mut [u8], patches: &[(usize, u8)]) {
    for &(index, byte) in patches {
        if let Some(slot) = bytes.get_mut(index) {
            *slot = byte;
        }
    }
}

/// Builds a replacement buffer and keeps the timing metadata of `source`.
///
/// A partial packet is forwarded with the buffer that completes it. Timestamps
/// from the earlier chunk are not copied.
fn timed_buffer(source: &gstreamer::Buffer, bytes: Vec<u8>) -> gstreamer::Buffer {
    let mut buffer = gstreamer::Buffer::from_slice(bytes);
    let Some(dest) = buffer.get_mut() else {
        return buffer;
    };
    dest.set_pts(source.pts());
    dest.set_dts(source.dts());
    dest.set_duration(source.duration());
    dest.set_offset(source.offset());
    dest.set_offset_end(source.offset_end());
    dest.set_flags(source.flags());
    buffer
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use gstreamer::glib::prelude::Cast;
    use gstreamer::prelude::{ElementExt, ElementExtManual, GstBinExt, PadExt};

    use super::*;

    #[test]
    fn flush_and_stream_start_reset_continuity_tracking() {
        assert!(event_resets_continuity(gstreamer::EventType::FlushStart));
        assert!(event_resets_continuity(gstreamer::EventType::FlushStop));
        assert!(event_resets_continuity(gstreamer::EventType::StreamStart));
        assert!(!event_resets_continuity(gstreamer::EventType::Caps));
        assert!(!event_resets_continuity(gstreamer::EventType::Eos));
    }

    /// Marker placed in the non-PUSI packet whose continuity counter resets.
    const RESET_MARKER: &[u8] = b"AFTER_RESET_MARKER";
    /// Payload that sits in the packet before the reset.
    const LEAD_MARKER: &[u8] = b"LEAD_BYTES_PRESENT";
    /// Payload of the following PES, which must survive either way.
    const NEXT_MARKER: &[u8] = b"NEXT_PES_PRESENT";

    #[test]
    fn tsdemux_keeps_payload_across_a_continuity_reset() {
        gstreamer::init().expect("gstreamer init");
        let stream = synthetic_reset_stream();
        let collected = {
            let guard = push_through_tsdemux(&stream);
            guard.lock().expect("collected buffers").clone()
        };
        assert!(
            contains(&collected, LEAD_MARKER),
            "tsdemux produced no elementary payload ({} bytes)",
            collected.len()
        );
        assert!(
            contains(&collected, RESET_MARKER),
            "tsdemux dropped the payload after the continuity reset ({} bytes)",
            collected.len()
        );
        assert!(
            contains(&collected, NEXT_MARKER),
            "tsdemux dropped the PES that follows the reset ({} bytes)",
            collected.len()
        );
    }

    fn contains(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|window| window == needle)
    }

    fn push_through_tsdemux(stream: &[u8]) -> Arc<Mutex<Vec<u8>>> {
        let pipeline = gstreamer::Pipeline::new();
        let appsrc = gstreamer_app::AppSrc::builder()
            .caps(
                &gstreamer::Caps::builder("video/mpegts")
                    .field("systemstream", true)
                    .build(),
            )
            .format(gstreamer::Format::Bytes)
            .build();
        let demux = gstreamer::ElementFactory::make("tsdemux")
            .build()
            .expect("tsdemux from gstreamer1.0-plugins-bad");
        let fakesink = gstreamer::ElementFactory::make("fakesink")
            .property("signal-handoffs", true)
            .property("sync", false)
            .build()
            .expect("fakesink");

        pipeline.add(&appsrc).expect("add appsrc");
        pipeline.add(&demux).expect("add tsdemux");
        pipeline.add(&fakesink).expect("add fakesink");
        appsrc
            .upcast_ref::<gstreamer::Element>()
            .link(&demux)
            .expect("link appsrc to tsdemux");

        let collected = Arc::new(Mutex::new(Vec::new()));
        let handoff_bytes = Arc::clone(&collected);
        let _handoff = fakesink.connect("handoff", false, move |values| {
            let buffer = values.get(1)?.get::<gstreamer::Buffer>().ok()?;
            let Ok(mapped) = buffer.map_readable() else {
                return None;
            };
            handoff_bytes
                .lock()
                .expect("handoff lock")
                .extend_from_slice(mapped.as_slice());
            None
        });

        let sink_pad = fakesink.static_pad("sink").expect("fakesink sink pad");
        demux.connect_pad_added(move |_demux, pad| {
            if pad.link(&sink_pad).is_err() {
                logger::warn!("mpeg-ts integration test failed to link a tsdemux pad");
            }
        });

        let demux_sink = demux.static_pad("sink").expect("tsdemux sink pad");
        let state = Arc::new(Mutex::new(MpegTsContinuityState::new()));
        demux_sink
            .add_probe(
                gstreamer::PadProbeType::BUFFER
                    | gstreamer::PadProbeType::EVENT_DOWNSTREAM
                    | gstreamer::PadProbeType::EVENT_FLUSH,
                move |_pad, info| repair_probe_info(info, &state, 0),
            )
            .expect("continuity probe");

        pipeline.set_state(gstreamer::State::Playing).expect("play");
        let _null = NullOnDrop(&pipeline);
        for chunk in stream.chunks(100) {
            appsrc
                .push_buffer(gstreamer::Buffer::from_slice(chunk.to_vec()))
                .expect("push mpeg-ts chunk");
        }
        appsrc.end_of_stream().expect("eos");
        wait_for_eos(&pipeline);
        collected
    }

    struct NullOnDrop<'a>(&'a gstreamer::Pipeline);

    impl Drop for NullOnDrop<'_> {
        fn drop(&mut self) {
            let _ = self.0.set_state(gstreamer::State::Null);
        }
    }

    fn wait_for_eos(pipeline: &gstreamer::Pipeline) {
        let bus = pipeline.bus().expect("pipeline bus");
        let message = bus.timed_pop_filtered(
            gstreamer::ClockTime::from_seconds(5),
            &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
        );
        match message {
            Some(message) => match message.view() {
                gstreamer::MessageView::Eos(..) => {}
                gstreamer::MessageView::Error(error) => {
                    panic!(
                        "tsdemux pipeline error: {} ({:?})",
                        error.error(),
                        error.debug()
                    );
                }
                _ => panic!("unexpected tsdemux bus message"),
            },
            None => panic!("timed out waiting for tsdemux eos"),
        }
    }

    fn synthetic_reset_stream() -> Vec<u8> {
        let mut stream = Vec::new();
        stream.extend(psi_packet(0x0000, &pat_section()));
        stream.extend(psi_packet(0x0100, &pmt_section()));
        stream.extend(elementary_packets());
        stream
    }

    fn pat_section() -> Vec<u8> {
        psi_section(
            0x00,
            &[
                0x00, 0x01, // transport_stream_id
                0xC1, // current version 0
                0x00, 0x00, // section numbers
                0x00, 0x01, // program 1
                0xE1, 0x00, // PMT PID 0x0100
            ],
        )
    }

    fn pmt_section() -> Vec<u8> {
        psi_section(
            0x02,
            &[
                0x00, 0x01, // program number
                0xC1, 0x00, 0x00, // version and section numbers
                0xFF, 0xFF, // no PCR
                0xF0, 0x00, // empty program info
                0x1B, // H.264
                0xE1, 0x01, // elementary PID 0x0101
                0xF0, 0x00, // empty ES info
            ],
        )
    }

    fn elementary_packets() -> Vec<u8> {
        let mut first_payload = vec![0x11; 184];
        first_payload[..14].copy_from_slice(&pes_header(0xE0));
        let lead_at = 14;
        first_payload[lead_at..lead_at + LEAD_MARKER.len()].copy_from_slice(LEAD_MARKER);

        let mut reset_payload = vec![0x22; 184];
        reset_payload[..RESET_MARKER.len()].copy_from_slice(RESET_MARKER);

        let mut next_payload = vec![0x33; 184];
        next_payload[..14].copy_from_slice(&pes_header(0xE0));
        next_payload[14..14 + NEXT_MARKER.len()].copy_from_slice(NEXT_MARKER);

        let mut packets = Vec::new();
        packets.extend(es_packet(0, true, &first_payload));
        // Same-fragment counters would be 0 then 1. Restart at 5 instead.
        packets.extend(es_packet(5, false, &reset_payload));
        packets.extend(es_packet(6, true, &next_payload));
        packets
    }

    fn pes_header(stream_id: u8) -> [u8; 14] {
        [
            0x00, 0x00, 0x01, stream_id, 0x00,
            0x00, // unbounded PES, flushed on the next unit start or EOS
            0x80, // MPEG marker bits
            0x80, // PTS only
            0x05, // PES header data length
            0x21, 0x00, 0x01, 0x00, 0x01, // PTS 0
        ]
    }

    fn es_packet(continuity: u8, unit_start: bool, payload: &[u8]) -> [u8; 188] {
        let mut packet = [0xFF; 188];
        packet[0] = 0x47;
        packet[1] = 0x01 | u8::from(unit_start) << 6;
        packet[2] = 0x01;
        packet[3] = 0x10 | (continuity & 0x0F);
        let len = payload.len().min(184);
        packet[4..4 + len].copy_from_slice(&payload[..len]);
        packet
    }

    fn psi_packet(pid: u16, section: &[u8]) -> [u8; 188] {
        let mut packet = [0xFF; 188];
        packet[0] = 0x47;
        packet[1] = 0x40 | u8::try_from((pid >> 8) & 0x1F).unwrap_or(0);
        packet[2] = u8::try_from(pid & 0xFF).unwrap_or(0);
        packet[3] = 0x10;
        packet[4] = 0x00;
        let len = section.len().min(183);
        packet[5..5 + len].copy_from_slice(&section[..len]);
        packet
    }

    fn psi_section(table_id: u8, after_length: &[u8]) -> Vec<u8> {
        let section_length = after_length.len() + 4;
        let mut section = Vec::new();
        section.push(table_id);
        section.push(0xB0 | u8::try_from((section_length >> 8) & 0x0F).unwrap_or(0));
        section.push(u8::try_from(section_length & 0xFF).unwrap_or(0));
        section.extend_from_slice(after_length);
        let crc = mpeg_crc32(&section);
        section.extend(crc.to_be_bytes());
        section
    }

    fn mpeg_crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= u32::from(byte) << 24;
            for _ in 0..8 {
                if crc & 0x8000_0000 != 0 {
                    crc = (crc << 1) ^ 0x04C1_1DB7;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }
}
