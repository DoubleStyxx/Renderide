//! A tablet pad's `group` event must not abort winit event-loop startup.

#![cfg(target_os = "linux")]

use std::io::{ErrorKind, Result as IoResult};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_manager_v2::{self, ZwpTabletManagerV2};
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2;
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_ring_v2::{
    self, ZwpTabletPadRingV2,
};
use wayland_protocols::wp::tablet::zv2::server::zwp_tablet_pad_strip_v2::{
    self, ZwpTabletPadStripV2,
};
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
use winit::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};
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

    let runtime = TempRuntime::create()?;
    let exe = std::env::current_exe().map_err(|err| format!("test executable: {err}"))?;
    let child = Command::new(exe)
        .arg("wayland_tablet_pad_group_does_not_abort_event_loop")
        .arg("--exact")
        .arg("--nocapture")
        .env(CHILD_ENV, "1")
        .env("XDG_RUNTIME_DIR", &runtime.path)
        .env("WAYLAND_DISPLAY", SOCKET_NAME)
        .env_remove("WAYLAND_SOCKET")
        .stdin(Stdio::null())
        .spawn()
        .map_err(|err| format!("spawn tablet-pad child: {err}"))?;

    let status = wait_for_child(child, Duration::from_secs(30))?;
    assert!(
        status.success(),
        "event loop aborted while a tablet pad was announced: {status}"
    );
    Ok(())
}

struct TempRuntime {
    path: PathBuf,
}

impl TempRuntime {
    fn create() -> Result<Self, String> {
        let path =
            std::env::temp_dir().join(format!("renderide-tablet-pad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&path)
            .map_err(|err| format!("create XDG_RUNTIME_DIR: {err}"))?;
        Ok(Self { path })
    }
}

impl Drop for TempRuntime {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn wait_for_child(mut child: std::process::Child, timeout: Duration) -> Result<ExitStatus, String> {
    let pid = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let status = child.wait();
        let _ = tx.send(status);
    });
    match rx.recv_timeout(timeout) {
        Ok(status) => status.map_err(|err| format!("wait for tablet-pad child: {err}")),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .status();
            let _ = rx.recv_timeout(Duration::from_secs(2));
            Err("tablet-pad child timed out".to_owned())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("tablet-pad child waiter disconnected".to_owned())
        }
    }
}

fn run_against_tablet_pad_compositor() -> Result<(), String> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (announced_tx, announced_rx) = mpsc::channel();
    let (shutdown_read, shutdown_write) =
        UnixStream::pair().map_err(|err| format!("shutdown pipe: {err}"))?;
    let server = thread::spawn(move || serve(announced_tx, ready_tx, shutdown_read));
    let client_result = (|| -> Result<(), String> {
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
        drive_until_pad_events(&mut event_loop, &mut app, &announced_rx)?;
        // Drop the client before the shutdown pipe so the compositor observes a
        // closed connection while it is still running, then wakes and exits.
        drop(event_loop);
        drop(shutdown_write);
        Ok(())
    })();

    let server_result = match server.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(format!("tablet compositor: {err}")),
        Err(_) => Err("tablet compositor thread panicked".to_owned()),
    };
    client_result?;
    server_result
}

fn drive_until_pad_events(
    event_loop: &mut EventLoop,
    app: &mut IdleApp,
    announced: &Receiver<()>,
) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        pump(event_loop, app)?;
        match announced.try_recv() {
            Ok(()) => {
                // The signal is sent after the server flushes, so this pump
                // dispatches the pad burst. Without the stub it aborts.
                return pump(event_loop, app);
            }
            Err(mpsc::TryRecvError::Empty) => {
                if Instant::now() >= deadline {
                    return Err("compositor never announced a tablet pad group".to_owned());
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("compositor stopped before announcing a tablet pad".to_owned());
            }
        }
    }
}

fn pump(event_loop: &mut EventLoop, app: &mut IdleApp) -> Result<(), String> {
    let status = event_loop.pump_app_events(Some(Duration::from_millis(50)), app);
    assert_eq!(
        status,
        PumpStatus::Continue,
        "event loop exited while a tablet pad was announced"
    );
    Ok(())
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
    announced: Sender<()>,
    pending_notice: bool,
    notified: bool,
}

struct TabletClient;

impl ClientData for TabletClient {}

fn serve(announced: Sender<()>, ready: Sender<()>, shutdown: UnixStream) -> IoResult<()> {
    let mut display = Display::<Compositor>::new().map_err(io_error)?;
    let listening = ListeningSocket::bind(SOCKET_NAME).map_err(io_error)?;
    register_globals(&display.handle());
    ready.send(()).map_err(io_error)?;
    let mut state = Compositor {
        announced,
        pending_notice: false,
        notified: false,
    };
    let started = Instant::now();
    let limit = Duration::from_secs(15);
    while started.elapsed() < limit {
        accept_clients(&listening, &display)?;
        let dispatched = match display.dispatch_clients(&mut state) {
            Ok(count) => count,
            Err(err) if is_disconnect(&err) => break,
            Err(err) => return Err(err),
        };
        display.flush_clients()?;
        if state.pending_notice && !state.notified {
            state.notified = true;
            state.announced.send(()).map_err(io_error)?;
        }
        if dispatched == 0 {
            let stop = wait_for_socket(
                &display,
                &listening,
                &shutdown,
                limit.saturating_sub(started.elapsed()),
            )?;
            if stop {
                break;
            }
        }
    }
    Ok(())
}

fn wait_for_socket(
    display: &Display<Compositor>,
    listening: &ListeningSocket,
    shutdown: &UnixStream,
    timeout: Duration,
) -> IoResult<bool> {
    let mut fds = [
        libc::pollfd {
            fd: display.as_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: listening.as_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: shutdown.as_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let timeout_ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: `fds` is a stack array of three initialized `pollfd` values. The
    // descriptors belong to `display`, `listening`, and `shutdown`, all borrowed
    // for this call, so they stay open until `poll` returns. `nfds` is 3.
    // `poll` does not retain the pointer.
    let ready = unsafe { libc::poll(fds.as_mut_ptr(), 3, timeout_ms) };
    if ready < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(err);
    }
    let hung_up = libc::POLLHUP | libc::POLLERR | libc::POLLNVAL;
    Ok(fds[2].revents != 0 || (fds[0].revents & hung_up) != 0)
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
    state: &mut Compositor,
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
    send_ring_events(&ring);
    send_strip_events(&strip);
    state.pending_notice = true;
}

fn send_ring_events(ring: &ZwpTabletPadRingV2) {
    ring.source(zwp_tablet_pad_ring_v2::Source::Finger);
    ring.angle(45.0);
    ring.frame(1);
    ring.stop();
    ring.frame(2);
}

fn send_strip_events(strip: &ZwpTabletPadStripV2) {
    strip.source(zwp_tablet_pad_strip_v2::Source::Finger);
    strip.position(32_768);
    strip.frame(1);
    strip.stop();
    strip.frame(2);
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
