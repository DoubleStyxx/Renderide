//! Shared control state for the RPC thread and the render thread.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, MutexGuard, mpsc};
use std::time::Instant;

use renderide_control_client::{
    CameraPoseParams, FrameStats, FramesWaitResult, KeyParams, MetaInfo, MouseClickParams,
    MouseMoveParams, MouseScrollParams, PlayPathParams,
};

use crate::frontend::input::WindowInputAccumulator;
use crate::shared::InputState;

use super::camera::{CameraDrive, validate_path, validate_pose};
#[cfg(test)]
use super::input::InputDebug;
use super::input::InputModel;

const DEFAULT_RING_CAPACITY: usize = 512;

/// One pending PNG capture.
pub(crate) struct CaptureJob {
    path: PathBuf,
    reply: mpsc::Sender<Result<u64, String>>,
}

impl CaptureJob {
    /// Completes the blocked `capture.png` call.
    pub(crate) fn complete(self, result: Result<u64, String>) {
        let _ = self.reply.send(result);
    }

    /// Destination path.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

struct State {
    shutdown: bool,
    input: InputModel,
    camera: CameraDrive,
    stats: VecDeque<FrameStats>,
    capacity: usize,
    presented: u64,
    wait_targets: Vec<u64>,
    meta: Option<MetaInfo>,
    capture: Option<CaptureJob>,
}

/// Render-thread and RPC-thread shared drone state.
pub(crate) struct ControlSession {
    state: Mutex<State>,
    cv: Condvar,
}

impl ControlSession {
    pub(crate) fn new() -> Self {
        Self::with_capacity(DEFAULT_RING_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            state: Mutex::new(State {
                shutdown: false,
                input: InputModel::default(),
                camera: CameraDrive::Idle,
                stats: VecDeque::new(),
                capacity: capacity.max(1),
                presented: 0,
                wait_targets: Vec::new(),
                meta: None,
                capture: None,
            }),
            cv: Condvar::new(),
        }
    }

    pub(crate) fn shutdown(&self) {
        let mut guard = self.lock();
        guard.shutdown = true;
        guard.capture.take();
        drop(guard);
        self.cv.notify_all();
    }

    pub(crate) fn mouse_move(&self, params: &MouseMoveParams) -> Result<(), String> {
        self.lock().input.move_pointer(params)
    }

    pub(crate) fn mouse_click(&self, params: &MouseClickParams) -> Result<(), String> {
        self.lock().input.click(params)
    }

    pub(crate) fn mouse_scroll(&self, params: &MouseScrollParams) -> Result<(), String> {
        self.lock().input.scroll(params)
    }

    pub(crate) fn key_down(&self, params: &KeyParams) -> Result<(), String> {
        self.lock().input.key_down(params)
    }

    pub(crate) fn key_up(&self, params: &KeyParams) -> Result<(), String> {
        self.lock().input.key_up(params)
    }

    #[cfg(test)]
    pub(crate) fn input_debug(&self) -> InputDebug {
        self.lock().input.debug()
    }

    pub(crate) fn take_input_state(&self) -> InputState {
        self.lock().input.take_input_state()
    }

    pub(crate) fn apply_to_window(&self, acc: &mut WindowInputAccumulator) {
        self.lock().input.apply_to_window(acc);
    }

    pub(crate) fn set_pose(&self, pose: CameraPoseParams) -> Result<(), String> {
        validate_pose(&pose)?;
        self.lock().camera = CameraDrive::Pose(pose);
        Ok(())
    }

    pub(crate) fn play_path(&self, params: PlayPathParams) -> Result<(), String> {
        validate_path(&params.keyframes, params.dt)?;
        self.lock().camera = CameraDrive::Path {
            keyframes: params.keyframes,
            dt: params.dt,
            started: Instant::now(),
        };
        Ok(())
    }

    pub(crate) fn current_pose(&self) -> Option<CameraPoseParams> {
        self.lock().camera.current(Instant::now())
    }

    pub(crate) fn set_meta_once(&self, meta: MetaInfo) {
        let mut guard = self.lock();
        if guard.meta.is_none() {
            guard.meta = Some(meta);
        }
    }

    pub(crate) fn meta(&self) -> Result<MetaInfo, String> {
        self.lock()
            .meta
            .clone()
            .ok_or_else(|| "meta is not available until the gpu device is initialized".to_owned())
    }

    pub(crate) fn reset_stats(&self) {
        self.lock().stats.clear();
    }

    pub(crate) fn query_stats(&self, since_frame: u64) -> Vec<FrameStats> {
        self.lock()
            .stats
            .iter()
            .filter(|sample| sample.frame_index >= since_frame)
            .cloned()
            .collect()
    }

    /// Records one presented frame and wakes `frames.wait` callers.
    pub(crate) fn note_presented(&self, mut stats: FrameStats) -> u64 {
        let mut guard = self.lock();
        let index = guard.presented;
        stats.frame_index = index;
        guard.presented = guard.presented.saturating_add(1);
        if guard.stats.len() == guard.capacity {
            guard.stats.pop_front();
        }
        guard.stats.push_back(stats);
        drop(guard);
        self.cv.notify_all();
        index
    }

    /// Blocks until `n` additional frames have been presented.
    pub(crate) fn wait_frames(&self, n: u64) -> Result<FramesWaitResult, String> {
        let mut guard = self.lock();
        if guard.shutdown {
            return Err("control api shut down".to_owned());
        }
        let target = guard.presented.saturating_add(n);
        guard.wait_targets.push(target);
        while guard.presented < target {
            if guard.shutdown {
                remove_target(&mut guard, target);
                return Err("control api shut down".to_owned());
            }
            guard = self.cv.wait(guard).unwrap_or_else(|err| err.into_inner());
        }
        remove_target(&mut guard, target);
        Ok(wait_result(guard.presented))
    }

    /// `true` once a `frames.wait` caller has latched `target`.
    #[cfg(test)]
    pub(crate) fn waiting_for(&self, target: u64) -> bool {
        self.lock().wait_targets.contains(&target)
    }

    #[cfg(test)]
    pub(crate) fn presented(&self) -> u64 {
        self.lock().presented
    }

    pub(crate) fn request_capture(&self, path: PathBuf) -> Result<u64, String> {
        let (sender, receiver) = mpsc::channel();
        {
            let mut guard = self.lock();
            if guard.shutdown {
                return Err("control api shut down".to_owned());
            }
            if guard.capture.is_some() {
                return Err("capture already in progress".to_owned());
            }
            guard.capture = Some(CaptureJob {
                path,
                reply: sender,
            });
        }
        self.cv.notify_all();
        match receiver.recv() {
            Ok(result) => result,
            Err(_) => Err("capture cancelled".to_owned()),
        }
    }

    pub(crate) fn take_capture(&self) -> Option<CaptureJob> {
        self.lock().capture.take()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|err| err.into_inner())
    }
}

fn wait_result(presented: u64) -> FramesWaitResult {
    FramesWaitResult {
        frame_index: presented.saturating_sub(1),
        presented,
    }
}

fn remove_target(guard: &mut State, target: u64) {
    if let Some(index) = guard.wait_targets.iter().position(|value| *value == target) {
        guard.wait_targets.remove(index);
    }
}
