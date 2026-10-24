//! Host-to-bootstrapper queue messages: heartbeat, clipboard, renderer spawn.
//!
//! Production code in this module is safe. Queue-loop tests that change the
//! process environment take [`crate::test_env::lock`].

#![deny(unsafe_code)]

mod command;
mod queue;

pub use command::{HostCommand, parse_host_command};
#[cfg(test)]
use queue::should_trace_iter;
pub use queue::{LoopAction, queue_loop};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_command_fixed_tokens() {
        assert_eq!(parse_host_command("HEARTBEAT"), HostCommand::Heartbeat);
        assert_eq!(parse_host_command("SHUTDOWN"), HostCommand::Shutdown);
        assert_eq!(parse_host_command("GETTEXT"), HostCommand::GetText);
    }

    #[test]
    fn parse_host_command_settext() {
        assert!(matches!(
            parse_host_command("SETTEXThello"),
            HostCommand::SetText(ref s) if s == "hello"
        ));
    }

    #[test]
    fn parse_host_command_renderer_args() {
        let cmd = parse_host_command("-QueueName q -QueueCapacity 4096");
        assert!(matches!(
            cmd,
            HostCommand::StartRenderer(ref launch)
                if launch.executable.is_none()
                    && launch.args
                        == vec!["-QueueName", "q", "-QueueCapacity", "4096"]
                            .into_iter()
                            .map(String::from)
                            .collect::<Vec<_>>()
        ));
    }

    #[test]
    fn parse_host_command_empty_message_is_start_renderer_empty() {
        assert!(matches!(
            parse_host_command(""),
            HostCommand::StartRenderer(ref launch)
                if launch.executable.is_none() && launch.args.is_empty()
        ));
    }

    #[test]
    fn parse_host_command_unwraps_quoted_start_renderer_wrapper() {
        let cmd = parse_host_command(
            r#"STARTRENDERER "C:\Program Files\renderide.exe" ARGS=-QueueName q -QueueCapacity 4096 -Extra "a b""#,
        );
        match cmd {
            HostCommand::StartRenderer(launch) => {
                assert_eq!(
                    launch.executable.as_deref(),
                    Some(r"C:\Program Files\renderide.exe")
                );
                assert_eq!(
                    launch.args,
                    vec![
                        "-QueueName".to_string(),
                        "q".to_string(),
                        "-QueueCapacity".to_string(),
                        "4096".to_string(),
                        "-Extra".to_string(),
                        "a b".to_string(),
                    ]
                );
            }
            other => panic!("expected renderer launch, got {other:?}"),
        }
    }

    #[test]
    fn parse_host_command_settext_only() {
        assert!(matches!(
            parse_host_command("SETTEXT"),
            HostCommand::SetText(ref s) if s.is_empty()
        ));
    }

    #[test]
    fn parse_host_command_settext_preserves_utf8_payload() {
        let cmd = parse_host_command("SETTEXTこんにちは");
        assert!(matches!(
            cmd,
            HostCommand::SetText(ref s) if s == "こんにちは"
        ));
    }

    #[test]
    fn parse_host_command_whitespace_only_yields_empty_start_renderer() {
        assert!(matches!(
            parse_host_command("   \t  "),
            HostCommand::StartRenderer(ref launch)
                if launch.executable.is_none() && launch.args.is_empty()
        ));
    }

    #[test]
    fn parse_host_command_unknown_token_becomes_start_renderer_argv() {
        let cmd = parse_host_command("CUSTOM opaque tail");
        assert!(matches!(
            cmd,
            HostCommand::StartRenderer(ref launch)
                if launch.executable.is_none()
                    && launch.args
                        == vec!["CUSTOM".to_string(), "opaque".to_string(), "tail".to_string()]
        ));
    }

    #[test]
    fn should_trace_iter_first_three_and_multiples_of_1000() {
        assert!(should_trace_iter(1));
        assert!(should_trace_iter(2));
        assert!(should_trace_iter(3));
        assert!(!should_trace_iter(4));
        assert!(!should_trace_iter(999));
        assert!(should_trace_iter(1000));
        assert!(!should_trace_iter(1001));
    }
}

#[cfg(test)]
#[expect(
    clippy::significant_drop_tightening,
    reason = "the queue loop reads RENDERIDE_INTERPROCESS_DIR, and the snapshot restores it, only while the process env lock is still held"
)]
mod queue_loop_tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    use super::queue_loop;
    use crate::child_lifetime::ChildLifetimeGroup;
    use crate::config::ResoBootConfig;
    use crate::ipc::{
        BootstrapQueues, RENDERIDE_INTERPROCESS_DIR_ENV,
        open_bootstrap_queues_host_publisher_first, open_bootstrap_queues_with_host_endpoints,
    };
    use crate::process_state::SharedChildSlot;

    #[test]
    fn queue_loop_returns_immediately_when_cancel_pre_set() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(&[RENDERIDE_INTERPROCESS_DIR_ENV]);
        let tmp =
            std::env::temp_dir().join(format!("bootstrapper_ql_cancel_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        guard.set_var(RENDERIDE_INTERPROCESS_DIR_ENV, &tmp);

        let prefix = format!("cc{}", std::process::id());
        let mut queues = BootstrapQueues::open(&prefix).expect("open queues");
        let config = ResoBootConfig::new(prefix, None, None).expect("config");
        let lifetime = ChildLifetimeGroup::new().expect("lifetime");
        let cancel = AtomicBool::new(true);
        let deadline = std::sync::Arc::new(Mutex::new(Instant::now()));
        let renderer = SharedChildSlot::empty();

        queue_loop(
            &mut queues.incoming,
            &mut queues.outgoing,
            &config,
            &cancel,
            &lifetime,
            &deadline,
            &renderer,
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn queue_loop_exits_on_shutdown_from_host_publisher() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(&[RENDERIDE_INTERPROCESS_DIR_ENV]);
        let tmp = std::env::temp_dir().join(format!("bootstrapper_ql_sd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        guard.set_var(RENDERIDE_INTERPROCESS_DIR_ENV, &tmp);

        let prefix = format!("sd{}", std::process::id());
        let (mut queues, mut host_publisher) =
            open_bootstrap_queues_host_publisher_first(&prefix).expect("open queues");

        assert!(
            host_publisher.try_enqueue(b"SHUTDOWN"),
            "host should enqueue SHUTDOWN before queue_loop runs"
        );

        let config = ResoBootConfig::new(prefix, None, None).expect("config");
        let lifetime = ChildLifetimeGroup::new().expect("lifetime");
        let cancel = AtomicBool::new(false);
        let deadline = std::sync::Arc::new(Mutex::new(Instant::now()));
        let renderer = SharedChildSlot::empty();

        queue_loop(
            &mut queues.incoming,
            &mut queues.outgoing,
            &config,
            &cancel,
            &lifetime,
            &deadline,
            &renderer,
        );

        assert!(
            cancel.load(std::sync::atomic::Ordering::SeqCst),
            "SHUTDOWN should set cancel"
        );

        drop(host_publisher);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn queue_loop_handles_heartbeat_then_shutdown() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(&[RENDERIDE_INTERPROCESS_DIR_ENV]);
        let tmp = std::env::temp_dir().join(format!("bootstrapper_ql_hb_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        guard.set_var(RENDERIDE_INTERPROCESS_DIR_ENV, &tmp);

        let prefix = format!("hb{}", std::process::id());
        let (mut queues, mut host_publisher) =
            open_bootstrap_queues_host_publisher_first(&prefix).expect("open queues");

        assert!(
            host_publisher.try_enqueue(b"HEARTBEAT"),
            "enqueue heartbeat"
        );
        assert!(host_publisher.try_enqueue(b"SHUTDOWN"), "enqueue shutdown");

        let config = ResoBootConfig::new(prefix, None, None).expect("config");
        let lifetime = ChildLifetimeGroup::new().expect("lifetime");
        let cancel = AtomicBool::new(false);
        let deadline = std::sync::Arc::new(Mutex::new(Instant::now()));
        let renderer = SharedChildSlot::empty();
        let initial_deadline = *deadline.lock().expect("lock");

        queue_loop(
            &mut queues.incoming,
            &mut queues.outgoing,
            &config,
            &cancel,
            &lifetime,
            &deadline,
            &renderer,
        );

        let final_deadline = *deadline.lock().expect("lock");
        assert!(
            final_deadline > initial_deadline,
            "HEARTBEAT must advance the shared deadline before SHUTDOWN exits the loop"
        );
        assert!(
            cancel.load(std::sync::atomic::Ordering::SeqCst),
            "SHUTDOWN must still set cancel after a preceding HEARTBEAT"
        );

        drop(host_publisher);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn queue_loop_handles_gettext_then_shutdown() {
        let guard = crate::test_env::lock();
        let _snap = guard.capture(&[RENDERIDE_INTERPROCESS_DIR_ENV]);
        let tmp = std::env::temp_dir().join(format!("bootstrapper_ql_gt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("mkdir");
        guard.set_var(RENDERIDE_INTERPROCESS_DIR_ENV, &tmp);

        let prefix = format!("gt{}", std::process::id());
        let (mut queues, mut host_publisher, mut host_subscriber) =
            open_bootstrap_queues_with_host_endpoints(&prefix).expect("open queues");

        assert!(host_publisher.try_enqueue(b"GETTEXT"), "enqueue gettext");
        assert!(host_publisher.try_enqueue(b"SHUTDOWN"), "enqueue shutdown");

        let config = ResoBootConfig::new(prefix, None, None).expect("config");
        let lifetime = ChildLifetimeGroup::new().expect("lifetime");
        let cancel = AtomicBool::new(false);
        let deadline = std::sync::Arc::new(Mutex::new(Instant::now()));
        let renderer = SharedChildSlot::empty();

        queue_loop(
            &mut queues.incoming,
            &mut queues.outgoing,
            &config,
            &cancel,
            &lifetime,
            &deadline,
            &renderer,
        );

        let mut received = false;
        for _ in 0..50 {
            if host_subscriber.try_dequeue().is_some() {
                received = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            received,
            "GETTEXT must produce an outgoing response before SHUTDOWN exits the loop"
        );
        assert!(
            cancel.load(std::sync::atomic::Ordering::SeqCst),
            "SHUTDOWN must still set cancel after a preceding GETTEXT"
        );

        drop(host_publisher);
        drop(host_subscriber);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
