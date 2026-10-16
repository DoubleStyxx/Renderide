//! Blocking TCP client.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::protocol::{
    Ack, CameraPoseParams, CapturePngParams, CapturePngResult, FrameStats, FramesWaitParams,
    FramesWaitResult, KeyParams, MetaInfo, MouseClickParams, MouseMoveParams, MouseScrollParams,
    PlayPathParams, StatsQueryParams, parse_response_line, request_line,
};

/// Failure from a control call.
#[derive(Debug)]
pub enum ClientError {
    /// Socket setup, write, or read failed.
    Io(std::io::Error),
    /// The response was not a JSON-RPC result.
    Protocol(String),
    /// The server returned an error object.
    Rpc {
        /// JSON-RPC error code.
        code: i64,
        /// Error text.
        message: String,
    },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "control io: {err}"),
            Self::Protocol(message) => write!(f, "control protocol: {message}"),
            Self::Rpc { code, message } => write!(f, "control rpc {code}: {message}"),
        }
    }
}

impl std::error::Error for ClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Protocol(_) | Self::Rpc { .. } => None,
        }
    }
}

impl From<std::io::Error> for ClientError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// Blocking client for one control connection.
pub struct ControlClient {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
}

impl ControlClient {
    /// Connects to `addr` (`127.0.0.1:port`).
    pub fn connect(addr: impl ToSocketAddrs) -> Result<Self, ClientError> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let reader_stream = stream.try_clone()?;
        Ok(Self {
            reader: BufReader::new(reader_stream),
            writer: stream,
            next_id: 1,
        })
    }

    /// `input.mouse_move`.
    pub fn mouse_move(&mut self, params: &MouseMoveParams) -> Result<Ack, ClientError> {
        self.call("input.mouse_move", params)
    }

    /// `input.mouse_click`.
    pub fn mouse_click(&mut self, params: &MouseClickParams) -> Result<Ack, ClientError> {
        self.call("input.mouse_click", params)
    }

    /// `input.mouse_scroll`.
    pub fn mouse_scroll(&mut self, params: &MouseScrollParams) -> Result<Ack, ClientError> {
        self.call("input.mouse_scroll", params)
    }

    /// `input.key_down`.
    pub fn key_down(&mut self, params: &KeyParams) -> Result<Ack, ClientError> {
        self.call("input.key_down", params)
    }

    /// `input.key_up`.
    pub fn key_up(&mut self, params: &KeyParams) -> Result<Ack, ClientError> {
        self.call("input.key_up", params)
    }

    /// `camera.set_pose`.
    pub fn set_pose(&mut self, params: &CameraPoseParams) -> Result<Ack, ClientError> {
        self.call("camera.set_pose", params)
    }

    /// `camera.play_path`.
    pub fn play_path(&mut self, params: &PlayPathParams) -> Result<Ack, ClientError> {
        self.call("camera.play_path", params)
    }

    /// `frames.wait`. Returns after `n` additional presented frames.
    pub fn frames_wait(&mut self, n: u64) -> Result<FramesWaitResult, ClientError> {
        self.call("frames.wait", &FramesWaitParams { n })
    }

    /// `stats.reset`.
    pub fn stats_reset(&mut self) -> Result<Ack, ClientError> {
        self.call("stats.reset", &EmptyParams)
    }

    /// `stats.query`.
    pub fn stats_query(&mut self, since_frame: u64) -> Result<Vec<FrameStats>, ClientError> {
        self.call("stats.query", &StatsQueryParams { since_frame })
    }

    /// `capture.png`. Blocks until the renderer writes the next presented frame.
    pub fn capture_png(
        &mut self,
        path: impl Into<String>,
    ) -> Result<CapturePngResult, ClientError> {
        self.call("capture.png", &CapturePngParams { path: path.into() })
    }

    /// `meta`.
    pub fn meta(&mut self) -> Result<MetaInfo, ClientError> {
        self.call("meta", &EmptyParams)
    }

    /// `scene.load`. The current renderer returns method-not-found.
    pub fn scene_load(&mut self, name: &str) -> Result<Value, ClientError> {
        self.call_value("scene.load", &SceneLoadParams { name })
    }

    /// Sends one request and returns the raw JSON-RPC result value.
    pub fn call_json(
        &mut self,
        method: &str,
        params: &impl Serialize,
    ) -> Result<Value, ClientError> {
        self.call_value(method, params)
    }

    fn call<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &impl Serialize,
    ) -> Result<T, ClientError> {
        let value = self.call_value(method, params)?;
        serde_json::from_value(value).map_err(|err| ClientError::Protocol(err.to_string()))
    }

    fn call_value(&mut self, method: &str, params: &impl Serialize) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let line = request_line(id, method, params)
            .map_err(|err| ClientError::Protocol(err.to_string()))?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.flush()?;
        self.read_matching(id)
    }

    fn read_matching(&mut self, id: u64) -> Result<Value, ClientError> {
        let expected = Value::from(id);
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = self.reader.read_line(&mut buf)?;
            if read == 0 {
                return Err(ClientError::Protocol(
                    "control connection closed".to_owned(),
                ));
            }
            let line = buf.trim();
            if line.is_empty() {
                continue;
            }
            let decoded = parse_response_line(line).map_err(ClientError::Protocol)?;
            if decoded.id.as_ref() != Some(&expected) {
                continue;
            }
            return match decoded.body {
                Ok(value) => Ok(value),
                Err(error) => Err(ClientError::Rpc {
                    code: error.code,
                    message: error.message,
                }),
            };
        }
    }
}

#[derive(Serialize)]
struct EmptyParams;

#[derive(Serialize)]
struct SceneLoadParams<'a> {
    name: &'a str,
}
