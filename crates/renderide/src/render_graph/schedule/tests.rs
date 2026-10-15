use super::*;

fn step(phase: PassPhase, pass_idx: usize, wave_idx: usize) -> ScheduleStep {
    let upload_phase = match phase {
        PassPhase::FrameGlobal => ScheduleUploadPhase::FrameGlobal,
        PassPhase::PerView => ScheduleUploadPhase::PerView,
    };
    ScheduleStep {
        phase,
        pass_idx,
        wave_idx,
        upload_phase,
    }
}

fn schedule(steps: Vec<ScheduleStep>, waves: Vec<std::ops::Range<usize>>) -> FrameSchedule {
    FrameSchedule::new(steps, waves, Vec::new(), Vec::new(), Vec::new(), Vec::new())
}

#[test]
fn frame_global_steps_filters_correctly() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::PerView, 1, 1),
            step(PassPhase::FrameGlobal, 2, 0),
            step(PassPhase::PerView, 3, 1),
        ],
        vec![0..2, 2..4],
    );
    let global: Vec<_> = sched.frame_global_steps().collect();
    assert_eq!(global.len(), 2);
    assert_eq!(global[0].pass_idx, 0);
    assert_eq!(global[1].pass_idx, 2);
    assert_eq!(sched.frame_global_pass_indices(), &[0usize, 2]);
    assert_eq!(sched.per_view_pass_indices(), &[1usize, 3]);
}

#[test]
fn per_view_steps_filters_correctly() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::PerView, 1, 1),
            step(PassPhase::PerView, 2, 1),
        ],
        vec![0..1, 1..3],
    );
    let per_view: Vec<_> = sched.per_view_steps().collect();
    assert_eq!(per_view.len(), 2);
    assert_eq!(per_view[0].pass_idx, 1);
    assert_eq!(per_view[1].pass_idx, 2);
}

#[test]
fn pass_count_and_wave_count() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::PerView, 1, 1),
            step(PassPhase::PerView, 2, 2),
        ],
        vec![0..1, 1..2, 2..3],
    );
    assert_eq!(sched.pass_count(), 3);
    assert_eq!(sched.wave_count(), 3);
}

#[test]
fn empty_schedule() {
    let sched = FrameSchedule::empty();
    assert_eq!(sched.pass_count(), 0);
    assert_eq!(sched.wave_count(), 0);
    assert_eq!(sched.frame_global_steps().count(), 0);
    assert_eq!(sched.per_view_steps().count(), 0);
    assert!(sched.frame_global_pass_indices().is_empty());
    assert!(sched.per_view_pass_indices().is_empty());
}

#[test]
fn validate_accepts_well_formed_schedule() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::PerView, 1, 1),
            step(PassPhase::PerView, 2, 1),
        ],
        vec![0..1, 1..3],
    );
    assert!(sched.validate().is_ok());
}

#[test]
fn validate_rejects_per_view_before_frame_global() {
    let sched = schedule(
        vec![
            step(PassPhase::PerView, 0, 0),
            step(PassPhase::FrameGlobal, 1, 0),
        ],
        core::iter::once(0..2).collect(),
    );
    let err = sched.validate().unwrap_err();
    assert!(matches!(
        err,
        ScheduleValidationError::FrameGlobalAfterPerView { .. }
    ));
}

#[test]
fn validate_rejects_wave_inversion() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 1),
            step(PassPhase::PerView, 1, 0),
        ],
        core::iter::once(0..2).collect(),
    );
    let err = sched.validate().unwrap_err();
    assert!(matches!(
        err,
        ScheduleValidationError::WaveOrderInverted { .. }
    ));
}

#[test]
fn validate_rejects_wave_range_gap() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::PerView, 1, 1),
        ],
        vec![0..1, 2..2],
    );
    let err = sched.validate().unwrap_err();
    assert!(matches!(err, ScheduleValidationError::WaveRangeGap { .. }));
}

#[test]
fn hud_snapshot_counts_phases_and_wave_sizes() {
    let sched = schedule(
        vec![
            step(PassPhase::FrameGlobal, 0, 0),
            step(PassPhase::FrameGlobal, 1, 0),
            step(PassPhase::PerView, 2, 1),
            step(PassPhase::PerView, 3, 1),
            step(PassPhase::PerView, 4, 2),
        ],
        vec![0..2, 2..4, 4..5],
    );
    let snap = ScheduleHudSnapshot::from_schedule(&sched);
    assert_eq!(snap.pass_count, 5);
    assert_eq!(snap.wave_count, 3);
    assert_eq!(snap.frame_global_count, 2);
    assert_eq!(snap.per_view_count, 3);
    assert_eq!(snap.passes_per_wave, vec![2, 2, 1]);
}
