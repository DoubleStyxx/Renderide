//! A tablet pad's `group` event must not abort winit event-loop startup.

#![cfg(target_os = "linux")]

use std::io::{ErrorKind, Result as IoResult};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_manager_v2::{self, ZwpTabletManagerV2};
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2;
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2;
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_v2::ZwpTabletPadV2;
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use wayland_protocols::xdg::shell::server::xdg_wm_base::XdgWmBase;
use wayland_server::backend::ClientData;
use wayland_server::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_region, wl_seat, wl_shm, wl_shm_pool,
    wl_surface, wl_touch,
};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, ListeningSocket, New,
    Resource,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::pump_events::EventLoopExtPumpEvents;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::wayland::{EventLoopBuilderExtWayland, EventLoopExtWayland};
use winit::window::WindowId;

const SOCKET_NAME: &str = "renderide-tablet-pad";
const CHILD_ENV: &str = "RENDERIDE_TABLET_PAD_CHILD";

#[test]
fn wayland_tablet_pad_group_does_not_abort_event_loop() -> Result<(), String> {
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_against_tablet_pad_compositor();
    }

    let runtime = std::env::temp_dir().join(format!("renderide-tablet-pad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&runtime);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&runtime)
        .map_err(|err| format!("create XDG_RUNTIME_DIR: {err}"))?;

    let exe = std::env::current_exe().map_err(|err| format!("test executable: {err}"))?;
    let mut child = Command::new(exe)
        .arg("wayland_tablet_pad_group_does_not_abort_event_loop")
        .arg("--exact")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("WAYLAND_DISPLAY", SOCKET_NAME)
        .env_remove("WAYLAND_SOCKET")
        .stdin(Stdio::null())
        .spawn()
        .map_err(|err| format!("spawn tablet-pad child: {err}"))?;

    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("poll child: {err}"))?
        {
            break status;
        }
        if started.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("tablet-pad child timed out".to_owned());
        }
        thread::sleep(Duration::from_millis(20));
    };
    let _ = std::fs::remove_dir_all(&runtime);
    assert!(
        status.success(),
        "event loop aborted while a tablet pad was announced: {status}"
    );
    Ok(())
}

fn run_against_tablet_pad_compositor() -> Result<(), String> {
    let announced = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();
    let server_announced = Arc::clone(&announced);
    let server = thread::spawn(move || serve(server_announced, ready_tx));
    ready_rx
        .recv_timeout(Duration::from_secs(2))
        .map_err(|err| format!("tablet compositor failed to bind: {err}"))?;

    let mut builder = EventLoop::builder();
    builder.with_wayland().with_any_thread(true);
    let mut event_loop = builder
        .build()
        .map_err(|err| format!("create wayland event loop: {err}"))?;
    if !event_loop.is_wayland() {
        return Err("tablet-pad regression must run on the Wayland backend".to_owned());
    }

    let mut app = IdleApp;
    let started = Instant::now();
    while !announced.load(Ordering::Acquire) {
        if started.elapsed() > Duration::from_secs(5) {
            return Err("compositor never announced a tablet pad group".to_owned());
        }
        pump(&mut event_loop, &mut app);
        thread::sleep(Duration::from_millis(5));
    }
    // The announcement flag is set before the server flushes. Drain the pad burst
    // after that flush. Without the stub this dispatch aborts the process.
    thread::sleep(Duration::from_millis(50));
    for _ in 0..10 {
        pump(&mut event_loop, &mut app);
    }
    drop(event_loop);

    let joined_at = Instant::now();
    while !server.is_finished() && joined_at.elapsed() < Duration::from_secs(2) {
        thread::sleep(Duration::from_millis(20));
    }
    if server.is_finished() {
        return match server.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(format!("tablet compositor: {err}")),
            Err(_) => Err("tablet compositor thread panicked".to_owned()),
        };
    }
    Ok(())
}

fn pump(event_loop: &mut EventLoop, app: &mut IdleApp) {
    let _ = event_loop.pump_app_events(Some(Duration::from_millis(20)), app);
}

struct IdleApp;

impl ApplicationHandler for IdleApp {
    fn can_create_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}

struct Compositor {
    announced: Arc<AtomicBool>,
}

struct TabletClient;

impl ClientData for TabletClient {}

fn serve(announced: Arc<AtomicBool>, ready: Sender<()>) -> IoResult<()> {
    let mut display = Display::<Compositor>::new().map_err(io_error)?;
    let listening = ListeningSocket::bind(SOCKET_NAME).map_err(io_error)?;
    register_globals(&display.handle());
    ready.send(()).map_err(io_error)?;
    let mut state = Compositor { announced };
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(15) {
        accept_clients(&listening, &display)?;
        match display.dispatch_clients(&mut state) {
            Ok(0) => thread::sleep(Duration::from_millis(2)),
            Ok(_) => {}
            Err(err) if is_disconnect(&err) => break,
            Err(err) => return Err(err),
        }
        display.flush_clients()?;
    }
    Ok(())
}

fn accept_clients(listening: &ListeningSocket, display: &Display<Compositor>) -> IoResult<()> {
    while let Some(stream) = listening.accept()? {
        insert_client(display, stream)?;
    }
    Ok(())
}

fn insert_client(display: &Display<Compositor>, stream: UnixStream) -> IoResult<()> {
    display
        .handle()
        .insert_client(stream, Arc::new(TabletClient))
        .map(|_| ())
}

fn register_globals(handle: &DisplayHandle) {
    handle.create_global::<Compositor, wl_compositor::WlCompositor, ()>(6, ());
    handle.create_global::<Compositor, wl_shm::WlShm, ()>(1, ());
    handle.create_global::<Compositor, wl_seat::WlSeat, ()>(7, ());
    handle.create_global::<Compositor, XdgWmBase, ()>(6, ());
    handle.create_global::<Compositor, ZwpTabletManagerV2, ()>(1, ());
}

fn is_disconnect(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::BrokenPipe | ErrorKind::ConnectionAborted | ErrorKind::ConnectionReset
    )
}

fn io_error<E: std::fmt::Display>(err: E) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

fn announce_pad(
    state: &Compositor,
    client: &Client,
    handle: &DisplayHandle,
    tablet_seat: &ZwpTabletSeatV2,
) {
    let version = tablet_seat.version();
    let Ok(pad) = client.create_resource::<ZwpTabletPadV2, (), Compositor>(handle, version, ())
    else {
        return;
    };
    tablet_seat.pad_added(&pad);
    let Ok(group) =
        client.create_resource::<ZwpTabletPadGroupV2, (), Compositor>(handle, version, ())
    else {
        return;
    };
    pad.group(&group);
    let Ok(ring) =
        client.create_resource::<ZwpTabletPadRingV2, (), Compositor>(handle, version, ())
    else {
        return;
    };
    group.ring(&ring);
    let Ok(strip) =
        client.create_resource::<ZwpTabletPadStripV2, (), Compositor>(handle, version, ())
    else {
        return;
    };
    group.strip(&strip);
    group.done();
    pad.done();
    state.announced.store(true, Ordering::Release);
}

macro_rules! ignore_dispatch {
    ($($iface:ty),+ $(,)?) => {$(
        impl Dispatch<$iface, ()> for Compositor {
            fn request(
                _state: &mut Self,
                _client: &Client,
                _resource: &$iface,
                _request: <$iface as Resource>::Request,
                _data: &(),
                _handle: &DisplayHandle,
                _data_init: &mut DataInit<'_, Self>,
            ) {
            }
        }
    )+};
}

ignore_dispatch!(
    wl_surface::WlSurface,
    wl_region::WlRegion,
    wl_buffer::WlBuffer,
    wl_touch::WlTouch,
    wl_pointer::WlPointer,
    wl_keyboard::WlKeyboard,
    ZwpTabletSeatV2,
    ZwpTabletPadV2,
    ZwpTabletPadGroupV2,
    ZwpTabletPadRingV2,
    ZwpTabletPadStripV2,
);

impl GlobalDispatch<wl_compositor::WlCompositor, ()> for Compositor {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_compositor::WlCompositor>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<wl_compositor::WlCompositor, ()> for Compositor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                data_init.init(id, ());
            }
            wl_compositor::Request::CreateRegion { id } => {
                data_init.init(id, ());
            }
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_shm::WlShm, ()> for Compositor {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_shm::WlShm>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let shm = data_init.init(resource, ());
        shm.format(wl_shm::Format::Argb8888);
        shm.format(wl_shm::Format::Xrgb8888);
    }
}

impl Dispatch<wl_shm::WlShm, ()> for Compositor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_shm::WlShm,
        request: wl_shm::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, .. } = request {
            data_init.init(id, ());
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for Compositor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_shm_pool::WlShmPool,
        request: wl_shm_pool::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm_pool::Request::CreateBuffer { id, .. } = request {
            data_init.init(id, ());
        }
    }
}

impl GlobalDispatch<wl_seat::WlSeat, ()> for Compositor {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_seat::WlSeat>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let seat = data_init.init(resource, ());
        seat.capabilities(wl_seat::Capability::Touch);
        seat.name("renderide-tablet-test".to_owned());
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Compositor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_seat::WlSeat,
        request: wl_seat::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_seat::Request::GetPointer { id } => {
                data_init.init(id, ());
            }
            wl_seat::Request::GetKeyboard { id } => {
                data_init.init(id, ());
            }
            wl_seat::Request::GetTouch { id } => {
                data_init.init(id, ());
            }
            wl_seat::Request::Release => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<XdgWmBase, ()> for Compositor {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgWmBase>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<XdgWmBase, ()> for Compositor {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &XdgWmBase,
        _request: <XdgWmBase as Resource>::Request,
        _data: &(),
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
    }
}

impl GlobalDispatch<ZwpTabletManagerV2, ()> for Compositor {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<ZwpTabletManagerV2>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpTabletManagerV2, ()> for Compositor {
    fn request(
        state: &mut Self,
        client: &Client,
        _resource: &ZwpTabletManagerV2,
        request: zwp_tablet_manager_v2::Request,
        _data: &(),
        handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_tablet_manager_v2::Request::GetTabletSeat { tablet_seat, .. } = request {
            let tablet_seat = data_init.init(tablet_seat, ());
            announce_pad(state, client, handle, &tablet_seat);
        }
    }
}
