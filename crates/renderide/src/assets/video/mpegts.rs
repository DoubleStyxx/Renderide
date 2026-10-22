//! Attaches the MPEG-TS continuity fix to playbin's `tsdemux` elements.
//!
//! See [`super::ts_continuity`] for why fragmented streams otherwise corrupt or
//! stall decoded video and audio.

use std::sync::{Arc, Mutex};

use gstreamer::prelude::{ElementExt, GstObjectExt, ObjectExt, PadExtManual};

use super::ts_continuity::{
    IgnoreContinuityProperty, MpegTsContinuityState, MpegTsContinuityStrategy, continuity_strategy,
    repair_mpegts_continuity,
};

/// `tsdemux` property added in GStreamer 1.30.
const IGNORE_CONTINUITY_PROPERTY: &str = "ignore-continuity-counter";

/// Connects playbin `element-setup` so every `tsdemux` tolerates fragment counter resets.
///
/// The handler is owned by `pipeline` and is released with it.
pub(super) fn attach_mpegts_continuity_fix(pipeline: &gstreamer::Element, asset_id: i32) {
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
    let state = Arc::new(Mutex::new(MpegTsContinuityState::default()));
    let probe = pad.add_probe(
        gstreamer::PadProbeType::BUFFER
            | gstreamer::PadProbeType::EVENT_DOWNSTREAM
            | gstreamer::PadProbeType::EVENT_FLUSH,
        move |_pad, info| {
            profiling::scope!("video::mpegts_continuity");
            repair_probe_info(info, &state);
            gstreamer::PadProbeReturn::Ok
        },
    );
    if probe.is_none() {
        logger::warn!("video texture {asset_id}: failed to install tsdemux continuity repair");
        return;
    }
    logger::info!("video texture {asset_id}: installed MPEG-TS continuity repair on tsdemux");
}

/// Resets counters on flush or stream start, and repairs buffer payloads.
fn repair_probe_info(info: &mut gstreamer::PadProbeInfo<'_>, state: &Mutex<MpegTsContinuityState>) {
    if let Some(event) = info.event()
        && event_resets_continuity(event.type_())
    {
        with_continuity_state(state, MpegTsContinuityState::reset);
        return;
    }
    with_continuity_state(state, |state| repair_probe_buffers(info, state));
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
fn with_continuity_state(
    state: &Mutex<MpegTsContinuityState>,
    body: impl FnOnce(&mut MpegTsContinuityState),
) {
    let mut guard = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            logger::warn!(
                "mpeg-ts continuity state lock poisoned; continuing with the recovered counters"
            );
            poisoned.into_inner()
        }
    };
    body(&mut guard);
}

/// Repairs a single buffer or every buffer in a list.
fn repair_probe_buffers(info: &mut gstreamer::PadProbeInfo<'_>, state: &mut MpegTsContinuityState) {
    if let Some(buffer) = info.buffer_mut() {
        repair_buffer(buffer.make_mut(), state);
        return;
    }
    let Some(list) = info.buffer_list_mut() else {
        return;
    };
    let list = list.make_mut();
    let count = list.len();
    for index in 0..count {
        if let Some(buffer) = list.get_mut(index) {
            repair_buffer(buffer, state);
        }
    }
}

/// Rewrites continuity counters in one mapped buffer.
fn repair_buffer(buffer: &mut gstreamer::BufferRef, state: &mut MpegTsContinuityState) {
    let Ok(mut mapped) = buffer.map_writable() else {
        return;
    };
    repair_mpegts_continuity(mapped.as_mut_slice(), state);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_and_stream_start_reset_continuity_tracking() {
        assert!(event_resets_continuity(gstreamer::EventType::FlushStart));
        assert!(event_resets_continuity(gstreamer::EventType::FlushStop));
        assert!(event_resets_continuity(gstreamer::EventType::StreamStart));
        assert!(!event_resets_continuity(gstreamer::EventType::Caps));
        assert!(!event_resets_continuity(gstreamer::EventType::Eos));
    }
}
