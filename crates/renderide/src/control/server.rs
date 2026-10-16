//! Newline-delimited JSON-RPC 2.0 server bound to `127.0.0.1`.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use renderide_control_client::{
    Ack, CODE_INTERNAL_ERROR, CODE_INVALID_PARAMS, CODE_METHOD_NOT_FOUND, CapturePngResult,
    ParsedRequest, SCENE_LOAD_TODO, error_line, from_params, method_scene_load, parse_request_line,
    success_line,
};
use serde_json::Value;

use super::session::ControlSession;

const MAX_LINE_BYTES: usize = 1_048_576;

/// Accept thread for the drone control API.
pub(crate) struct ControlServer {
    session: Arc<ControlSession>,
    local_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

impl ControlServer {
    /// Binds `127.0.0.1:port`. Port `0` selects an ephemeral port.
    pub(crate) fn bind(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
        let local_addr = listener.local_addr()?;
        let session = Arc::new(ControlSession::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let session_thread = Arc::clone(&session);
        let shutdown_thread = Arc::clone(&shutdown);
        let accept_thread = thread::Builder::new()
            .name("renderide-control-accept".to_owned())
            .spawn(move || accept_loop(listener, session_thread, shutdown_thread))?;
        Ok(Self {
            session,
            local_addr,
            shutdown,
            accept_thread: Some(accept_thread),
        })
    }

    pub(crate) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub(crate) fn session(&self) -> Arc<ControlSession> {
        Arc::clone(&self.session)
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        self.session.shutdown();
        let _ = TcpStream::connect(self.local_addr);
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(listener: TcpListener, session: Arc<ControlSession>, shutdown: Arc<AtomicBool>) {
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if shutdown.load(Ordering::Acquire) {
                    break;
                }
                let session = Arc::clone(&session);
                let shutdown = Arc::clone(&shutdown);
                let _ = thread::Builder::new()
                    .name("renderide-control-conn".to_owned())
                    .spawn(move || serve_connection(stream, session, shutdown));
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => {
                if !shutdown.load(Ordering::Acquire) {
                    logger::warn!("control api accept failed: {err}");
                }
                break;
            }
        }
    }
}

fn serve_connection(stream: TcpStream, session: Arc<ControlSession>, shutdown: Arc<AtomicBool>) {
    let _ = stream.set_nodelay(true);
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    loop {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        line.clear();
        match read_line_limited(&mut reader, &mut line) {
            Ok(ReadLine::Eof | ReadLine::TooLong) | Err(_) => break,
            Ok(ReadLine::Empty) => {}
            Ok(ReadLine::Line) => {
                if let Some(response) = dispatch_line(&line, &session)
                    && (writer.write_all(response.as_bytes()).is_err() || writer.flush().is_err())
                {
                    break;
                }
            }
        }
    }
}

enum ReadLine {
    Line,
    Empty,
    Eof,
    TooLong,
}

fn read_line_limited(
    reader: &mut BufReader<TcpStream>,
    buf: &mut String,
) -> std::io::Result<ReadLine> {
    let read = reader.read_line(buf)?;
    if read == 0 {
        return Ok(ReadLine::Eof);
    }
    if buf.len() > MAX_LINE_BYTES {
        return Ok(ReadLine::TooLong);
    }
    if buf.trim().is_empty() {
        return Ok(ReadLine::Empty);
    }
    Ok(ReadLine::Line)
}

fn dispatch_line(line: &str, session: &ControlSession) -> Option<String> {
    match parse_request_line(line.trim()) {
        Ok(request) => dispatch_request(request, session),
        Err(failure) => Some(error_line(
            &failure.id.clone().unwrap_or(Value::Null),
            failure.code,
            &failure.message,
        )),
    }
}

fn dispatch_request(request: ParsedRequest, session: &ControlSession) -> Option<String> {
    let id = request.id.clone()?;
    Some(handle(&id, &request, session))
}

fn handle(id: &Value, request: &ParsedRequest, session: &ControlSession) -> String {
    match request.method.as_str() {
        "input.mouse_move" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.mouse_move(&params)),
        ),
        "input.mouse_click" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.mouse_click(&params)),
        ),
        "input.mouse_scroll" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.mouse_scroll(&params)),
        ),
        "input.key_down" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.key_down(&params)),
        ),
        "input.key_up" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.key_up(&params)),
        ),
        "camera.set_pose" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.set_pose(params)),
        ),
        "camera.play_path" => ack_call(
            id,
            from_params(&request.params).and_then(|params| session.play_path(params)),
        ),
        "frames.wait" => frames_wait(id, session, &request.params),
        "stats.reset" => {
            session.reset_stats();
            success_or_internal(id, &Ack::ok())
        }
        "stats.query" => stats_query(id, session, &request.params),
        "capture.png" => capture_png(id, session, &request.params),
        "meta" => meta(id, session),
        method if method == method_scene_load() => {
            error_line(id, CODE_METHOD_NOT_FOUND, SCENE_LOAD_TODO)
        }
        other => error_line(
            id,
            CODE_METHOD_NOT_FOUND,
            &format!("Method not found: {other}"),
        ),
    }
}

fn ack_call(id: &Value, result: Result<(), String>) -> String {
    match result {
        Ok(()) => success_or_internal(id, &Ack::ok()),
        Err(message) => error_line(id, CODE_INVALID_PARAMS, &message),
    }
}

fn frames_wait(id: &Value, session: &ControlSession, params: &Value) -> String {
    let params = match parse::<renderide_control_client::FramesWaitParams>(params) {
        Ok(params) => params,
        Err(message) => return error_line(id, CODE_INVALID_PARAMS, &message),
    };
    match session.wait_frames(params.n) {
        Ok(result) => success_or_internal(id, &result),
        Err(message) => error_line(id, CODE_INTERNAL_ERROR, &message),
    }
}

fn stats_query(id: &Value, session: &ControlSession, params: &Value) -> String {
    let params = match parse::<renderide_control_client::StatsQueryParams>(params) {
        Ok(params) => params,
        Err(message) => return error_line(id, CODE_INVALID_PARAMS, &message),
    };
    success_or_internal(id, &session.query_stats(params.since_frame))
}

fn capture_png(id: &Value, session: &ControlSession, params: &Value) -> String {
    let params = match parse::<renderide_control_client::CapturePngParams>(params) {
        Ok(params) => params,
        Err(message) => return error_line(id, CODE_INVALID_PARAMS, &message),
    };
    if params.path.is_empty() {
        return error_line(id, CODE_INVALID_PARAMS, "capture path must not be empty");
    }
    match session.request_capture(PathBuf::from(&params.path)) {
        Ok(frame_index) => success_or_internal(
            id,
            &CapturePngResult {
                path: params.path,
                frame_index,
            },
        ),
        Err(message) => error_line(id, CODE_INTERNAL_ERROR, &message),
    }
}

fn meta(id: &Value, session: &ControlSession) -> String {
    match session.meta() {
        Ok(meta) => success_or_internal(id, &meta),
        Err(message) => error_line(id, CODE_INTERNAL_ERROR, &message),
    }
}

fn success_or_internal(id: &Value, result: &impl serde::Serialize) -> String {
    match success_line(id, result) {
        Ok(line) => line,
        Err(err) => error_line(id, CODE_INTERNAL_ERROR, &err.to_string()),
    }
}

fn parse<T: serde::de::DeserializeOwned>(params: &Value) -> Result<T, String> {
    from_params(params)
}
