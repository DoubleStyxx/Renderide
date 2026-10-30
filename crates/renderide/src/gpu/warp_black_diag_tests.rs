use std::path::Path;

use super::{
    FrameAccum, FrameLogState, HoldDecision, LOAD_CLEAR, PipelineNote, WarpExtraWait, fault_flags,
    path_requests_frame_log,
};

#[test]
fn fault_flags_detect_device_removed_tdr_and_timeout() {
    let removed = fault_flags("DXGI_ERROR_DEVICE_REMOVED (0x887A0005)");
    assert!(removed.dxgi_device_removed);
    assert!(!removed.tdr);

    let hung = fault_flags("DXGI_ERROR_DEVICE_HUNG");
    assert!(!hung.dxgi_device_removed);
    assert!(hung.tdr);
    assert!(hung.timeout);

    let tdr = fault_flags("Timeout Detection and Recovery");
    assert!(tdr.tdr);
    assert!(tdr.timeout);

    let plain = fault_flags("wgpu validation error: binding mismatch");
    assert!(!plain.dxgi_device_removed);
    assert!(!plain.tdr);
    assert!(!plain.timeout);
}

#[test]
fn warp_hold_waits_two_ready_frames_after_the_watchdog_resumes() {
    let mut wait = WarpExtraWait::default();
    assert_eq!(wait.withhold(false, 0, 1), HoldDecision::NotWarp);
    assert_eq!(wait.withhold(true, 4, 1), HoldDecision::PipelinesNotReady);
    assert_eq!(wait.withhold(true, 0, 0), HoldDecision::WatchdogNotQuiet);
    assert_eq!(
        wait.withhold(true, 0, 1),
        HoldDecision::Withhold { frame: 1 }
    );
    assert_eq!(
        wait.withhold(true, 0, 1),
        HoldDecision::Withhold { frame: 2 }
    );
    assert_eq!(wait.withhold(true, 0, 1), HoldDecision::PublishAfterWait);
    assert_eq!(
        wait.withhold(true, 0, 2),
        HoldDecision::Withhold { frame: 1 }
    );
}

#[test]
fn frame_log_emits_on_state_change_and_counts_the_previous_streak() {
    let mut log = FrameLogState::new();
    let first = log
        .observe(sample_signature(3, "stem@1:0x2"))
        .expect("first");
    assert_eq!(first.frame, 1);
    assert_eq!(first.prev_streak, 0);
    assert!(log.observe(sample_signature(3, "stem@1:0x2")).is_none());
    let changed = log
        .observe(sample_signature(0, "stem@1:0x2"))
        .expect("change");
    assert_eq!(changed.frame, 3);
    assert_eq!(changed.prev_streak, 2);
    assert_eq!(changed.signature.forward_draws, 0);
    assert!(changed.signature.pipelines.contains("stem@1:0x2"));
}

#[test]
fn pipeline_signature_is_sorted_and_caps_the_logged_list() {
    let mut first = FrameAccum::new();
    let mut second = FrameAccum::new();
    for (kind, bits) in [("b", 2u32), ("a", 1), ("c", 3)] {
        first.pipelines.insert(pipeline(kind, bits));
    }
    for (kind, bits) in [("c", 3u32), ("a", 1), ("b", 2)] {
        second.pipelines.insert(pipeline(kind, bits));
    }
    assert_eq!(first.signature(), second.signature());
    for bits in 0..20 {
        first
            .pipelines
            .insert(pipeline(&format!("p{bits:02}"), bits));
    }
    let signature = first.signature();
    assert_eq!(
        signature.pipeline_overflow,
        u64::try_from(first.pipelines.len() - 16).unwrap()
    );
    assert_eq!(signature.pipelines.matches(';').count(), 15);
}

#[test]
fn pbs_lit_output_path_arms_the_frame_log() {
    assert!(path_requests_frame_log(Path::new(
        "suites/pbs_lit_material_matrix/capture.png"
    )));
    assert!(!path_requests_frame_log(Path::new(
        "suites/unlit_textured/capture.png"
    )));
}

fn sample_signature(forward_draws: u64, pipelines: &str) -> super::FrameSignature {
    super::FrameSignature {
        forward_draws,
        forward_instances: forward_draws,
        depth_draws: 0,
        depth_instances: 0,
        pre_cull: 4,
        frustum: 1,
        hiz: 0,
        visible: 3,
        views: 1,
        pipelines: pipelines.to_owned(),
        pipeline_overflow: 0,
        color_load: LOAD_CLEAR,
        depth_load: LOAD_CLEAR,
        skybox_draws: 0,
        skybox_kinds: String::new(),
        forward_draws_detail: "-".to_owned(),
        depth_draws_detail: "-".to_owned(),
        forward_draw_overflow: 0,
        depth_draw_overflow: 0,
    }
}

fn pipeline(kind: &str, bits: u32) -> PipelineNote {
    PipelineNote {
        kind: kind.to_owned(),
        shader_asset_id: 7,
        variant_bits: bits,
    }
}
