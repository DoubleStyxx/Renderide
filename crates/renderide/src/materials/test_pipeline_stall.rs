//! Debug-only delay before the first non-Null material pipeline build.
//!
//! Release builds compile this out. `RENDERIDE_TEST_PIPELINE_STALL_MS` is otherwise inert.
//! A stall longer than the headless shutdown grace leaves the Null checker (linear 0.05,
//! encoded 63) on screen unless the capture waits for `renderide_pipeline_pending` to hit 0.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::pipeline_kind::RasterPipelineKind;

/// Sleeps once, on the first non-Null build, when the test env is set.
#[cfg(debug_assertions)]
pub(super) fn stall_non_null_material_pipeline(kind: &RasterPipelineKind) {
    let Some(ms) = take_stall_ms(kind) else {
        return;
    };
    logger::info!("Test pipeline stall: {ms} ms before the first non-Null material pipeline build");
    std::thread::sleep(Duration::from_millis(ms));
}

/// Release builds do not read the env and do not sleep.
#[cfg(not(debug_assertions))]
pub(super) fn stall_non_null_material_pipeline(_kind: &RasterPipelineKind) {}

#[cfg(debug_assertions)]
fn take_stall_ms(kind: &RasterPipelineKind) -> Option<u64> {
    static CONSUMED: AtomicBool = AtomicBool::new(false);
    let configured = configured_stall_ms();
    let already_consumed = CONSUMED.load(Ordering::Relaxed);
    let ms = stall_duration_ms(kind, configured, already_consumed)?;
    CONSUMED.store(true, Ordering::Relaxed);
    Some(ms)
}

#[cfg(debug_assertions)]
fn configured_stall_ms() -> u64 {
    static MS: OnceLock<u64> = OnceLock::new();
    *MS.get_or_init(|| parse_stall_ms(std::env::var("RENDERIDE_TEST_PIPELINE_STALL_MS").ok()))
}

fn parse_stall_ms(raw: Option<String>) -> u64 {
    raw.as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(0)
}

/// Milliseconds to sleep for this build. `None` means do not sleep.
fn stall_duration_ms(
    kind: &RasterPipelineKind,
    configured_ms: u64,
    already_consumed: bool,
) -> Option<u64> {
    if already_consumed || configured_ms == 0 || matches!(kind, RasterPipelineKind::Null) {
        None
    } else {
        Some(configured_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn stem() -> RasterPipelineKind {
        RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default"))
    }

    #[test]
    fn stall_hits_the_first_non_null_pipeline_only() {
        assert_eq!(stall_duration_ms(&stem(), 12_000, false), Some(12_000));
        assert_eq!(stall_duration_ms(&stem(), 12_000, true), None);
        assert_eq!(
            stall_duration_ms(&RasterPipelineKind::Null, 12_000, false),
            None
        );
        assert_eq!(stall_duration_ms(&stem(), 0, false), None);
    }

    #[test]
    fn stall_env_parses_milliseconds_and_rejects_blank() {
        assert_eq!(parse_stall_ms(Some("12000".to_string())), 12_000);
        assert_eq!(parse_stall_ms(Some("  50 ".to_string())), 50);
        assert_eq!(parse_stall_ms(Some("nope".to_string())), 0);
        assert_eq!(parse_stall_ms(Some(String::new())), 0);
        assert_eq!(parse_stall_ms(None), 0);
    }
}
