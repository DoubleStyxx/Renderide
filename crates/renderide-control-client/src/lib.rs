//! Blocking JSON-RPC 2.0 client for the renderide dev-only drone control API.
//!
//! The renderer listens on `127.0.0.1` when it is built with `--features control-api`
//! and started with `--control-port <port>`. Messages are one JSON object per line.
//! This crate depends only on serde.

#![forbid(unsafe_code)]

mod client;
mod protocol;

pub use client::{ClientError, ControlClient};
pub use protocol::{
    Ack, CODE_INTERNAL_ERROR, CODE_INVALID_PARAMS, CODE_INVALID_REQUEST, CODE_METHOD_NOT_FOUND,
    CODE_PARSE_ERROR, CameraPoseParams, CapturePngParams, CapturePngResult, CpuPhaseMs, FrameStats,
    FramesWaitParams, FramesWaitResult, KeyParams, MetaInfo, MouseClickParams, MouseMoveParams,
    MouseScrollParams, ParsedRequest, PlayPathParams, RequestParseFailure, RpcErrorObject,
    SCENE_LOAD_TODO, StatsQueryParams, error_line, from_params, method_scene_load,
    parse_request_line, success_line,
};
