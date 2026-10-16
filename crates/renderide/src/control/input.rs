//! Control-injected pointer and keyboard state.

use renderide_control_client::{KeyParams, MouseClickParams, MouseMoveParams, MouseScrollParams};

use crate::frontend::input::WindowInputAccumulator;
use crate::shared::{InputState, Key, KeyboardState, MouseState, WindowState};

/// Pointer and keyboard state owned by the control session.
#[derive(Clone, Debug, Default)]
pub(crate) struct InputModel {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    scroll_x: f32,
    scroll_y: f32,
    left: bool,
    right: bool,
    middle: bool,
    button4: bool,
    button5: bool,
    pointer_touched: bool,
    buttons_touched: bool,
    keys: Vec<Key>,
    injected_keys: Vec<Key>,
    keys_touched: bool,
}

/// Snapshot used by tests.
#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InputDebug {
    /// Window x.
    pub x: f32,
    /// Window y.
    pub y: f32,
    /// Pending horizontal delta.
    pub dx: f32,
    /// Pending vertical delta.
    pub dy: f32,
    /// Pending horizontal scroll.
    pub scroll_x: f32,
    /// Pending vertical scroll.
    pub scroll_y: f32,
    /// Left button held.
    pub left: bool,
    /// Held host keys.
    pub keys: Vec<Key>,
}

impl InputModel {
    pub(crate) fn move_pointer(&mut self, params: &MouseMoveParams) -> Result<(), String> {
        require_finite("x", params.x)?;
        require_finite("y", params.y)?;
        if !params.dx.is_finite() || !params.dy.is_finite() {
            return Err("mouse delta must be finite".to_owned());
        }
        match params.x {
            Some(x) => self.x = x,
            None => self.x += params.dx,
        }
        match params.y {
            Some(y) => self.y = y,
            None => self.y += params.dy,
        }
        self.dx += params.dx;
        self.dy += params.dy;
        self.pointer_touched = true;
        Ok(())
    }

    pub(crate) fn click(&mut self, params: &MouseClickParams) -> Result<(), String> {
        let button = parse_button(&params.button)?;
        self.set_button(button, params.pressed);
        self.buttons_touched = true;
        self.pointer_touched = true;
        Ok(())
    }

    pub(crate) fn scroll(&mut self, params: &MouseScrollParams) -> Result<(), String> {
        if !params.dx.is_finite() || !params.dy.is_finite() {
            return Err("scroll delta must be finite".to_owned());
        }
        self.scroll_x += params.dx;
        self.scroll_y += params.dy;
        self.pointer_touched = true;
        Ok(())
    }

    pub(crate) fn key_down(&mut self, params: &KeyParams) -> Result<(), String> {
        let key = parse_key(&params.key)?;
        if !self.keys.contains(&key) {
            self.keys.push(key);
        }
        self.keys_touched = true;
        Ok(())
    }

    pub(crate) fn key_up(&mut self, params: &KeyParams) -> Result<(), String> {
        let key = parse_key(&params.key)?;
        self.keys.retain(|held| *held != key);
        self.keys_touched = true;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn debug(&self) -> InputDebug {
        InputDebug {
            x: self.x,
            y: self.y,
            dx: self.dx,
            dy: self.dy,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
            left: self.left,
            keys: self.keys.clone(),
        }
    }

    /// Builds a host [`InputState`] and clears one-shot deltas.
    pub(crate) fn take_input_state(&mut self) -> InputState {
        let position = glam::Vec2::new(self.x, self.y);
        let mouse = MouseState {
            is_active: self.pointer_touched,
            left_button_state: self.left,
            right_button_state: self.right,
            middle_button_state: self.middle,
            button4_state: self.button4,
            button5_state: self.button5,
            desktop_position: position,
            window_position: position,
            direct_delta: glam::Vec2::new(self.dx, self.dy),
            scroll_wheel_delta: glam::Vec2::new(self.scroll_x, self.scroll_y),
        };
        self.clear_deltas();
        InputState {
            mouse: Some(mouse),
            keyboard: Some(KeyboardState {
                type_delta: None,
                held_keys: self.keys.clone(),
                composition_active: false,
                composition_text: None,
            }),
            window: Some(WindowState {
                is_window_focused: true,
                is_fullscreen: false,
                window_resolution: glam::IVec2::ZERO,
                resolution_settings_applied: false,
                drag_and_drop_event: None,
            }),
            ..InputState::default()
        }
    }

    /// Merges injected input into the winit accumulator and clears one-shot deltas.
    pub(crate) fn apply_to_window(&mut self, acc: &mut WindowInputAccumulator) {
        if self.pointer_touched {
            acc.window_position = glam::Vec2::new(self.x, self.y);
            acc.mouse_active = true;
            acc.mouse_delta += glam::Vec2::new(self.dx, self.dy);
            acc.scroll_delta += glam::Vec2::new(self.scroll_x, self.scroll_y);
        }
        if self.buttons_touched {
            acc.left_held = self.left;
            acc.right_held = self.right;
            acc.middle_held = self.middle;
            acc.button4_held = self.button4;
            acc.button5_held = self.button5;
        }
        if self.keys_touched {
            sync_keys(acc, &mut self.injected_keys, &self.keys);
        }
        self.clear_deltas();
    }

    fn clear_deltas(&mut self) {
        self.dx = 0.0;
        self.dy = 0.0;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
    }

    fn set_button(&mut self, button: Button, pressed: bool) {
        match button {
            Button::Left => self.left = pressed,
            Button::Right => self.right = pressed,
            Button::Middle => self.middle = pressed,
            Button::Button4 => self.button4 = pressed,
            Button::Button5 => self.button5 = pressed,
        }
    }
}

fn sync_keys(acc: &mut WindowInputAccumulator, injected: &mut Vec<Key>, now: &[Key]) {
    for key in injected.iter() {
        if !now.contains(key) {
            acc.held_keys.retain(|held| held != key);
        }
    }
    for key in now {
        if !acc.held_keys.contains(key) {
            acc.held_keys.push(*key);
        }
    }
    injected.clear();
    injected.extend_from_slice(now);
}

fn require_finite(name: &str, value: Option<f32>) -> Result<(), String> {
    if value.is_some_and(|component| !component.is_finite()) {
        return Err(format!("mouse {name} must be finite"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Button {
    Left,
    Right,
    Middle,
    Button4,
    Button5,
}

fn parse_button(name: &str) -> Result<Button, String> {
    let button = match name.trim().to_ascii_lowercase().as_str() {
        "left" => Button::Left,
        "right" => Button::Right,
        "middle" => Button::Middle,
        "button4" => Button::Button4,
        "button5" => Button::Button5,
        _ => return Err(format!("unknown mouse button '{name}'")),
    };
    Ok(button)
}

fn parse_key(name: &str) -> Result<Key, String> {
    let trimmed = name.trim();
    if let Some(key) = single_char_key(trimmed) {
        return Ok(key);
    }
    named_key(trimmed).ok_or_else(|| format!("unknown key '{name}'"))
}

fn single_char_key(name: &str) -> Option<Key> {
    let mut chars = name.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    match ch {
        'a'..='z' => Some(letter(u8::try_from(ch as u32 - 'a' as u32).unwrap_or(0))),
        'A'..='Z' => Some(letter(u8::try_from(ch as u32 - 'A' as u32).unwrap_or(0))),
        '0'..='9' => Some(digit(u8::try_from(ch as u32 - '0' as u32).unwrap_or(0))),
        _ => None,
    }
}

fn letter(index: u8) -> Key {
    match index {
        0 => Key::A,
        1 => Key::B,
        2 => Key::C,
        3 => Key::D,
        4 => Key::E,
        5 => Key::F,
        6 => Key::G,
        7 => Key::H,
        8 => Key::I,
        9 => Key::J,
        10 => Key::K,
        11 => Key::L,
        12 => Key::M,
        13 => Key::N,
        14 => Key::O,
        15 => Key::P,
        16 => Key::Q,
        17 => Key::R,
        18 => Key::S,
        19 => Key::T,
        20 => Key::U,
        21 => Key::V,
        22 => Key::W,
        23 => Key::X,
        24 => Key::Y,
        _ => Key::Z,
    }
}

fn digit(index: u8) -> Key {
    match index {
        0 => Key::Alpha0,
        1 => Key::Alpha1,
        2 => Key::Alpha2,
        3 => Key::Alpha3,
        4 => Key::Alpha4,
        5 => Key::Alpha5,
        6 => Key::Alpha6,
        7 => Key::Alpha7,
        8 => Key::Alpha8,
        _ => Key::Alpha9,
    }
}

fn named_key(name: &str) -> Option<Key> {
    const NAMED: &[(&str, Key)] = &[
        ("space", Key::Space),
        ("enter", Key::Return),
        ("return", Key::Return),
        ("escape", Key::Escape),
        ("esc", Key::Escape),
        ("tab", Key::Tab),
        ("backspace", Key::Backspace),
        ("delete", Key::Delete),
        ("left", Key::LeftArrow),
        ("right", Key::RightArrow),
        ("up", Key::UpArrow),
        ("down", Key::DownArrow),
        ("leftarrow", Key::LeftArrow),
        ("rightarrow", Key::RightArrow),
        ("uparrow", Key::UpArrow),
        ("downarrow", Key::DownArrow),
        ("shift", Key::Shift),
        ("leftshift", Key::LeftShift),
        ("rightshift", Key::RightShift),
        ("control", Key::Control),
        ("ctrl", Key::Control),
        ("leftcontrol", Key::LeftControl),
        ("rightcontrol", Key::RightControl),
        ("alt", Key::Alt),
        ("leftalt", Key::LeftAlt),
        ("rightalt", Key::RightAlt),
        ("home", Key::Home),
        ("end", Key::End),
        ("pageup", Key::PageUp),
        ("pagedown", Key::PageDown),
        ("insert", Key::Insert),
        ("f1", Key::F1),
        ("f2", Key::F2),
        ("f3", Key::F3),
        ("f4", Key::F4),
        ("f5", Key::F5),
        ("f6", Key::F6),
        ("f7", Key::F7),
        ("f8", Key::F8),
        ("f9", Key::F9),
        ("f10", Key::F10),
        ("f11", Key::F11),
        ("f12", Key::F12),
    ];
    let folded = name.to_ascii_lowercase();
    NAMED
        .iter()
        .find(|(label, _)| *label == folded)
        .map(|(_, key)| *key)
}
