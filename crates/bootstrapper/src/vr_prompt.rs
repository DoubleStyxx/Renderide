//! Host argv augmentation so `FrooxEngine` receives `-Screen` or `-Device <HeadOutputDevice>`
//! before process startup, matching `FrooxEngine` `LaunchOptions` handling of `-screen` / `-device`.
//!
//! The renderer learns the effective device from IPC `RendererInitData` after connect.
//!
//! The interactive desktop/VR selection dialog itself lives in the bin-only `dialog` module
//! (`crates/bootstrapper/src/dialog.rs`). Keeping the `rfd` dependency out of this library
//! is what allows the bootstrapper lib unit-test executable to start on Windows; otherwise
//! its static import of `comctl32.dll`'s `TaskDialogIndirect` (added by `rfd`'s
//! `common-controls-v6` feature) would fail to resolve and abort the test exe with
//! `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) before any test could run.

use std::env;

/// When set, the bootstrapper does not show the desktop vs VR dialog (automation / headless).
pub const ENV_SKIP_VR_DIALOG: &str = "RENDERIDE_SKIP_VR_DIALOG";

/// Forwards to [`crate::cli::normalize_flag_token`].
#[cfg(test)]
fn normalized_flag_token(arg: &str) -> String {
    crate::cli::normalize_flag_token(arg)
}

/// Returns `true` when `args` already tell Resonite which output to use.
///
/// Recognizes `-Screen`, `-Device` (including `-Device=SteamVR` and `-Device:Screen360`),
/// legacy switches such as `-SteamVR` and `-RiftTouch`, and a bare `none`. A value that
/// merely contains those words, such as `-Data screen`, does not count. Bare `none` after a
/// boolean switch such as `-Invisible` still counts; see
/// [`crate::launch_args::host_args_specify_output_mode`].
pub(crate) fn host_args_have_explicit_output_device(args: &[String]) -> bool {
    crate::launch_args::host_args_specify_output_mode(args)
}

/// On Linux, returns `true` when at least one of `DISPLAY` (X11) or `WAYLAND_DISPLAY` (Wayland)
/// is set in the environment. On other platforms returns `true` unconditionally.
///
/// Used by [`should_prompt_vr_dialog`] to skip the dialog on headless / TTY launches where
/// GTK cannot open a window and `rfd::MessageDialog::show()` would block forever without
/// writing any log.
fn linux_graphical_session_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        let has_x11 = env::var_os("DISPLAY").is_some_and(|v| !v.is_empty());
        let has_wayland = env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
        has_x11 || has_wayland
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Removes `DISPLAY` and `WAYLAND_DISPLAY` from the process environment when they are set
/// to an empty string, on Linux.
///
/// `WAYLAND_DISPLAY=""` is sometimes used as a folk idiom for "force X11", but GTK3/GTK4
/// (which `rfd`'s zenity backend uses) treats the variable as *present* and tries to open a
/// Wayland display with an empty name; the connection fails and GTK does not transparently
/// fall back to X11. Stripping empty-valued display variables makes the env match the
/// "unset" case, so downstream GUI subprocesses (zenity, kdialog, winit, OpenXR) inherit a
/// clean environment and select an actually-working backend.
///
/// No-op on non-Linux targets.
pub fn sanitize_linux_display_env() {
    #[cfg(target_os = "linux")]
    {
        for key in ["WAYLAND_DISPLAY", "DISPLAY"] {
            if env::var_os(key).is_some_and(|v| v.is_empty()) {
                #[cfg(test)]
                crate::test_env::assert_shared_env_lock_held();
                // SAFETY: `env::remove_var` mutates the process environment. On Unix,
                // overlapping `set_var`, `remove_var`, or `getenv` is undefined behavior.
                // The launcher calls this on the main thread during early bootstrap, before
                // it spawns worker threads. Test builds also require `PROCESS_ENV_LOCK`
                // (checked above): that is the only mutex used by bootstrapper and
                // interprocess test helpers that mutate the environment or read it to
                // restore a previous value, so those helpers do not overlap this call.
                // Other code may still call `std::env::var` or `var_os`.
                unsafe {
                    env::remove_var(key);
                }
                logger::info!(
                    "Removed empty {key} from process environment so GUI subprocesses fall back \
                     to a working display backend.",
                );
            }
        }
    }
}

/// Whether the optional Yes/No dialog should run before spawning the Host.
///
/// Returns `false` when explicit output flags are already present, `CI` is set,
/// [`ENV_SKIP_VR_DIALOG`] is set, or (on Linux) neither `DISPLAY` nor `WAYLAND_DISPLAY`
/// is set -- the latter case is logged at `warn` level so headless launches are not silent.
pub(crate) fn should_prompt_vr_dialog(host_args: &[String]) -> bool {
    if host_args_have_explicit_output_device(host_args) {
        return false;
    }
    if env::var("CI").is_ok() {
        return false;
    }
    if env::var(ENV_SKIP_VR_DIALOG).is_ok() {
        return false;
    }
    if !linux_graphical_session_available() {
        logger::warn!(
            "Skipping desktop/VR dialog: neither DISPLAY nor WAYLAND_DISPLAY is set. \
             Pass -Screen or -Device SteamVR, or set {ENV_SKIP_VR_DIALOG}=1 to silence this warning.",
        );
        return false;
    }
    true
}

/// Prepends `-Device SteamVR` or `-Screen` to the Host argv list.
pub(crate) fn apply_host_vr_choice(host_args: Vec<String>, vr: bool) -> Vec<String> {
    if vr {
        let mut out = Vec::with_capacity(host_args.len().saturating_add(2));
        out.push("-Device".into());
        out.push("SteamVR".into());
        out.extend(host_args);
        out
    } else {
        let mut out = Vec::with_capacity(host_args.len().saturating_add(1));
        out.push("-Screen".into());
        out.extend(host_args);
        out
    }
}

#[cfg(test)]
#[expect(
    clippy::significant_drop_tightening,
    reason = "dialog decisions read the environment, and the snapshot restores it, only while the process env lock is still held"
)]
mod tests {
    use super::*;

    #[test]
    fn detects_screen_flags() {
        assert!(host_args_have_explicit_output_device(&["-Screen".into()]));
        assert!(host_args_have_explicit_output_device(&["-screen".into()]));
    }

    #[test]
    fn detects_device_flags() {
        assert!(host_args_have_explicit_output_device(&[
            "-Device".into(),
            "SteamVR".into()
        ]));
    }

    #[test]
    fn no_false_positives() {
        assert!(!host_args_have_explicit_output_device(&[
            "-Invisible".into(),
            "-Data".into()
        ]));
    }

    #[test]
    fn apply_vr_prepends_device_steamvr() {
        let out = apply_host_vr_choice(vec!["-Invisible".into()], true);
        assert_eq!(out, vec!["-Device", "SteamVR", "-Invisible"]);
    }

    #[test]
    fn apply_desktop_prepends_screen() {
        let out = apply_host_vr_choice(vec![], false);
        assert_eq!(out, vec!["-Screen"]);
    }

    #[test]
    fn normalized_flag_token_trims_and_strips_leading_dash() {
        assert_eq!(normalized_flag_token("  -Screen  "), "screen");
        assert_eq!(normalized_flag_token("Device"), "device");
        // Only one leading `-` is stripped; `--` prefixes remain normalized for the remainder.
        assert_eq!(normalized_flag_token("--Foo"), "-foo");
    }

    const VR_ENV_KEYS: &[&str] = &[ENV_SKIP_VR_DIALOG, "CI", "DISPLAY", "WAYLAND_DISPLAY"];

    #[test]
    fn should_prompt_false_when_ci_set() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.set_var("CI", "1");
        assert!(!should_prompt_vr_dialog(&[]));
    }

    #[test]
    fn should_prompt_false_when_skip_env_set() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.set_var(ENV_SKIP_VR_DIALOG, "1");
        assert!(!should_prompt_vr_dialog(&[]));
    }

    #[test]
    fn should_prompt_false_for_attached_device_form() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        guard.set_var("DISPLAY", ":0");
        assert!(!should_prompt_vr_dialog(&["-Device=SteamVR".into()]));
        assert!(!should_prompt_vr_dialog(&["-SteamVR".into()]));
        assert!(!should_prompt_vr_dialog(&["none".into()]));
        assert!(should_prompt_vr_dialog(&["-Data".into(), "screen".into()]));
    }

    #[test]
    fn should_prompt_false_when_device_explicit() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        assert!(!should_prompt_vr_dialog(&["-Device".into(), "x".into()]));
    }

    #[test]
    fn should_prompt_true_when_unset_and_display_present() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        guard.set_var("DISPLAY", ":0");
        guard.remove_var("WAYLAND_DISPLAY");
        assert!(should_prompt_vr_dialog(&[]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn should_prompt_false_on_linux_when_no_display() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        guard.remove_var("DISPLAY");
        guard.remove_var("WAYLAND_DISPLAY");
        assert!(!should_prompt_vr_dialog(&[]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn should_prompt_true_on_linux_with_wayland_only() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        guard.remove_var("DISPLAY");
        guard.set_var("WAYLAND_DISPLAY", "wayland-0");
        assert!(should_prompt_vr_dialog(&[]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn should_prompt_false_on_linux_when_display_is_empty_string() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.remove_var("CI");
        guard.remove_var(ENV_SKIP_VR_DIALOG);
        guard.set_var("DISPLAY", "");
        guard.set_var("WAYLAND_DISPLAY", "");
        assert!(!should_prompt_vr_dialog(&[]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sanitize_removes_empty_display_vars() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.set_var("DISPLAY", "");
        guard.set_var("WAYLAND_DISPLAY", "");
        sanitize_linux_display_env();
        assert!(env::var_os("DISPLAY").is_none());
        assert!(env::var_os("WAYLAND_DISPLAY").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sanitize_preserves_non_empty_display_vars() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.set_var("DISPLAY", ":0");
        guard.set_var("WAYLAND_DISPLAY", "wayland-0");
        sanitize_linux_display_env();
        assert_eq!(env::var_os("DISPLAY").as_deref(), Some(":0".as_ref()));
        assert_eq!(
            env::var_os("WAYLAND_DISPLAY").as_deref(),
            Some("wayland-0".as_ref())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sanitize_only_strips_empty_var_when_other_is_set() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(VR_ENV_KEYS);
        guard.set_var("DISPLAY", ":0");
        guard.set_var("WAYLAND_DISPLAY", "");
        sanitize_linux_display_env();
        assert_eq!(env::var_os("DISPLAY").as_deref(), Some(":0".as_ref()));
        assert!(env::var_os("WAYLAND_DISPLAY").is_none());
    }
}
