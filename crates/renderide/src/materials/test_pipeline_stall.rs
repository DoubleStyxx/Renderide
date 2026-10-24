//! Debug-only deferral of one non-Null material pipeline build.
//!
//! `RENDERIDE_TEST_PIPELINE_STALL_MS` arms a deadline on the first non-Null build.
//! Until that deadline the build stays on the render-thread queue, so the frame
//! can be published with `renderide_pipeline_pending > 0`. The render thread does
//! not sleep, and release builds do not read the env.

use super::pipeline_kind::RasterPipelineKind;

#[cfg(debug_assertions)]
pub(super) fn hold_render_thread_build(
    kind: &RasterPipelineKind,
    key: &super::cache::MaterialPipelineCacheKey,
) -> bool {
    let configured_ms = configured_stall_ms();
    if !stall_applies(kind, configured_ms) {
        return false;
    }
    static ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static HELD: std::sync::OnceLock<HeldBuild> = std::sync::OnceLock::new();
    if arm_once(&ARMED) {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(configured_ms);
        let _ = HELD.set(HeldBuild {
            key: key.clone(),
            until,
        });
        logger::info!(
            "Test pipeline stall: deferring one non-Null material pipeline for {configured_ms} ms"
        );
        return true;
    }
    match HELD.get() {
        Some(held) => {
            let elapsed_ms = elapsed_since_arm(held.until, configured_ms);
            hold_armed_build(&held.key == key, configured_ms, elapsed_ms)
        }
        None => false,
    }
}

#[cfg(debug_assertions)]
fn elapsed_since_arm(until: std::time::Instant, configured_ms: u64) -> u64 {
    let now = std::time::Instant::now();
    if now < until {
        let remaining =
            u64::try_from(until.saturating_duration_since(now).as_millis()).unwrap_or(u64::MAX);
        configured_ms.saturating_sub(remaining)
    } else {
        configured_ms
    }
}

#[cfg(not(debug_assertions))]
pub(super) fn hold_render_thread_build(
    _kind: &RasterPipelineKind,
    _key: &super::cache::MaterialPipelineCacheKey,
) -> bool {
    false
}

#[cfg(debug_assertions)]
struct HeldBuild {
    key: super::cache::MaterialPipelineCacheKey,
    until: std::time::Instant,
}

#[cfg(debug_assertions)]
fn configured_stall_ms() -> u64 {
    static MS: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *MS.get_or_init(|| parse_stall_ms(std::env::var("RENDERIDE_TEST_PIPELINE_STALL_MS").ok()))
}

#[cfg(any(debug_assertions, test))]
fn arm_once(armed: &std::sync::atomic::AtomicBool) -> bool {
    use std::sync::atomic::Ordering;
    !armed.swap(true, Ordering::AcqRel)
}

#[cfg(any(debug_assertions, test))]
fn parse_stall_ms(raw: Option<String>) -> u64 {
    raw.as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(0)
}

/// `true` when this kind is a non-Null build and a stall is configured.
#[cfg(any(debug_assertions, test))]
fn stall_applies(kind: &RasterPipelineKind, configured_ms: u64) -> bool {
    configured_ms > 0 && !matches!(kind, RasterPipelineKind::Null)
}

/// The armed build stays deferred until `elapsed_ms` reaches the configured stall.
#[cfg(any(debug_assertions, test))]
fn hold_armed_build(is_armed_key: bool, configured_ms: u64, elapsed_ms: u64) -> bool {
    is_armed_key && elapsed_ms < configured_ms
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::*;

    fn stem() -> RasterPipelineKind {
        RasterPipelineKind::EmbeddedStem(Arc::from("pbsmetallic_default"))
    }

    #[test]
    fn stall_applies_to_a_configured_non_null_pipeline_only() {
        assert!(stall_applies(&stem(), 12_000));
        assert!(!stall_applies(&RasterPipelineKind::Null, 12_000));
        assert!(!stall_applies(&stem(), 0));
    }

    #[test]
    fn armed_build_stays_deferred_until_the_deadline() {
        assert!(hold_armed_build(true, 12_000, 0));
        assert!(hold_armed_build(true, 12_000, 11_999));
        assert!(!hold_armed_build(true, 12_000, 12_000));
        assert!(!hold_armed_build(false, 12_000, 0));
    }

    #[test]
    fn swap_arms_the_stall_once() {
        let armed = AtomicBool::new(false);
        assert!(arm_once(&armed));
        assert!(!arm_once(&armed));
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
