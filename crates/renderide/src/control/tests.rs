//! Client/server roundtrip and headless `frames.wait` determinism.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use renderide_control_client::{
    CODE_METHOD_NOT_FOUND, CameraPoseParams, ClientError, ControlClient, KeyParams, MetaInfo,
    MouseClickParams, MouseMoveParams, MouseScrollParams, PlayPathParams, SCENE_LOAD_TODO,
};

use super::headless_tick_presents;
use super::server::ControlServer;
use super::session::ControlSession;
use crate::shared::Key;

fn zero_stats() -> renderide_control_client::FrameStats {
    renderide_control_client::FrameStats {
        frame_index: 0,
        cpu_frame_ms: 1.0,
        cpu_phase_ms: renderide_control_client::CpuPhaseMs {
            draw_prep: 0.25,
            cull: 0.1,
            upload: 0.05,
            submit: 0.02,
        },
        gpu_pass_ms: None,
        draw_calls: 3,
        instances: 3,
        upload_count: 1,
        upload_bytes: 128,
        triangles: 6,
    }
}

fn pose() -> CameraPoseParams {
    CameraPoseParams {
        pos: [1.0, 2.0, 3.0],
        rot: [0.0, 0.0, 0.0, 1.0],
        fov: 60.0,
    }
}

fn meta() -> MetaInfo {
    MetaInfo {
        adapter: "Test Adapter".to_owned(),
        driver: "test-driver".to_owned(),
        backend: "Vulkan".to_owned(),
        present_mode: "AutoNoVsync".to_owned(),
        resolution: [256, 256],
        git_sha: "01234567".to_owned(),
    }
}

fn wait_until(pred: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if pred() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn client_server_roundtrip_covers_methods() {
    let server = ControlServer::bind(0).expect("bind");
    let addr = server.local_addr();
    let session = server.session();
    session.set_meta_once(meta());

    let mut client = ControlClient::connect(addr).expect("connect");
    let got = client.meta().expect("meta");
    assert_eq!(got, meta());
    assert_eq!(client.meta().expect("meta again"), meta());

    client
        .mouse_move(&MouseMoveParams {
            x: Some(4.0),
            y: Some(5.0),
            dx: 1.0,
            dy: -2.0,
        })
        .expect("move");
    client
        .mouse_click(&MouseClickParams {
            button: "left".to_owned(),
            pressed: true,
        })
        .expect("click");
    client
        .mouse_scroll(&MouseScrollParams { dx: 0.0, dy: 3.0 })
        .expect("scroll");
    client
        .key_down(&KeyParams {
            key: "W".to_owned(),
        })
        .expect("down");
    client
        .key_up(&KeyParams {
            key: "A".to_owned(),
        })
        .expect("up");
    let input = session.input_debug();
    assert_eq!(input.x, 4.0);
    assert_eq!(input.y, 5.0);
    assert_eq!(input.dx, 1.0);
    assert!(input.left);
    assert_eq!(input.keys, vec![Key::W]);
    assert_eq!(input.scroll_y, 3.0);

    client.set_pose(&pose()).expect("pose");
    let current = session.current_pose().expect("current");
    assert_eq!(current.pos, pose().pos);
    client
        .play_path(&PlayPathParams {
            keyframes: vec![pose()],
            dt: 0.016,
        })
        .expect("path");
    assert_eq!(session.current_pose().expect("path pose").fov, 60.0);

    session.note_presented(zero_stats());
    session.note_presented(zero_stats());
    let all = client.stats_query(0).expect("query");
    assert_eq!(all.len(), 2);
    assert!(all[0].gpu_pass_ms.is_none());
    assert_eq!(all[0].frame_index, 0);
    assert_eq!(all[1].triangles, 6);
    assert_eq!(all[1].cpu_phase_ms.draw_prep, 0.25);
    let tail = client.stats_query(1).expect("since");
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0].frame_index, 1);
    client.stats_reset().expect("reset");
    assert!(client.stats_query(0).expect("empty").is_empty());

    let err = client.scene_load("world").expect_err("scene");
    match err {
        ClientError::Rpc { code, message } => {
            assert_eq!(code, CODE_METHOD_NOT_FOUND);
            assert_eq!(message, SCENE_LOAD_TODO);
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn unknown_method_is_not_found() {
    let server = ControlServer::bind(0).expect("bind");
    let mut client = ControlClient::connect(server.local_addr()).expect("connect");
    let err = client
        .call_json("no.such", &serde_json::json!({}))
        .expect_err("missing");
    match err {
        ClientError::Rpc { code, message } => {
            assert_eq!(code, CODE_METHOD_NOT_FOUND);
            assert!(message.contains("no.such"));
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn capture_png_completes_when_the_render_thread_finishes_the_job() {
    let server = ControlServer::bind(0).expect("bind");
    let session = server.session();
    let addr = server.local_addr();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut client = ControlClient::connect(addr).expect("connect");
        let _ = sender.send(client.capture_png("/tmp/frame.png"));
    });
    let job = wait_for_capture(&session);
    assert_eq!(job.path(), std::path::Path::new("/tmp/frame.png"));
    job.complete(Ok(4));
    let result = receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("reply")
        .expect("png");
    assert_eq!(result.frame_index, 4);
    assert_eq!(result.path, "/tmp/frame.png");
}

#[test]
fn frames_wait_counts_only_presented_headless_frames() {
    assert!(!headless_tick_presents(false, false));
    assert!(!headless_tick_presents(true, true));
    assert!(headless_tick_presents(true, false));

    let server = ControlServer::bind(0).expect("bind");
    let session = server.session();
    let mut immediate = ControlClient::connect(server.local_addr()).expect("connect");
    let zero = immediate.frames_wait(0).expect("zero");
    assert_eq!(zero.presented, 0);
    assert_eq!(zero.frame_index, 0);

    let addr = server.local_addr();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut client = ControlClient::connect(addr).expect("connect");
        let _ = sender.send(client.frames_wait(3));
    });
    assert!(wait_until(|| session.waiting_for(3)));

    for _ in 0..4 {
        if headless_tick_presents(false, false) {
            session.note_presented(zero_stats());
        }
        if headless_tick_presents(true, true) {
            session.note_presented(zero_stats());
        }
    }
    assert!(session.waiting_for(3));
    assert!(receiver.try_recv().is_err());

    note_presented_headless(&session);
    note_presented_headless(&session);
    assert!(session.waiting_for(3));
    note_presented_headless(&session);

    let result = receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("wait")
        .expect("ok");
    assert_eq!(result.presented, 3);
    assert_eq!(result.frame_index, 2);
    assert!(!session.waiting_for(3));
    assert_eq!(session.presented(), 3);
}

fn note_presented_headless(session: &ControlSession) {
    if headless_tick_presents(true, false) {
        session.note_presented(zero_stats());
    }
}

fn wait_for_capture(session: &ControlSession) -> super::session::CaptureJob {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if let Some(job) = session.take_capture() {
            return job;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("capture job was not queued");
}
