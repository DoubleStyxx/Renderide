//! Keyboard shortcut detection for the windowed driver.

use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

/// Returns whether `event` is the Alt+Enter (or Alt+NumpadEnter) borderless-fullscreen toggle.
pub(super) fn fullscreen_toggle_shortcut(event: &WindowEvent, modifiers: ModifiersState) -> bool {
    let WindowEvent::KeyboardInput {
        event,
        is_synthetic,
        ..
    } = event
    else {
        return false;
    };
    fullscreen_toggle_shortcut_from_parts(
        event.physical_key,
        event.state,
        event.repeat,
        *is_synthetic,
        modifiers,
    )
}

fn fullscreen_toggle_shortcut_from_parts(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
    is_synthetic: bool,
    modifiers: ModifiersState,
) -> bool {
    !is_synthetic
        && !repeat
        && state == ElementState::Pressed
        && modifiers.alt_key()
        && matches!(
            physical_key,
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
        )
}

/// Returns whether `event` is the F7 imgui-visibility toggle.
pub(super) fn imgui_visibility_shortcut(event: &WindowEvent) -> bool {
    let WindowEvent::KeyboardInput {
        event,
        is_synthetic,
        ..
    } = event
    else {
        return false;
    };
    imgui_visibility_shortcut_from_parts(
        event.physical_key,
        event.state,
        event.repeat,
        *is_synthetic,
    )
}

fn imgui_visibility_shortcut_from_parts(
    physical_key: PhysicalKey,
    state: ElementState,
    repeat: bool,
    is_synthetic: bool,
) -> bool {
    !is_synthetic
        && !repeat
        && state == ElementState::Pressed
        && physical_key == PhysicalKey::Code(KeyCode::F7)
}

#[cfg(test)]
mod tests {
    use winit::event::ElementState;
    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

    use super::{fullscreen_toggle_shortcut_from_parts, imgui_visibility_shortcut_from_parts};

    #[test]
    fn fullscreen_toggle_shortcut_accepts_alt_enter_and_alt_numpad_enter() {
        assert!(fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Pressed,
            false,
            false,
            ModifiersState::ALT,
        ));
        assert!(fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::NumpadEnter),
            ElementState::Pressed,
            false,
            false,
            ModifiersState::ALT,
        ));
    }

    #[test]
    fn fullscreen_toggle_shortcut_rejects_missing_alt_repeat_release_synthetic_and_other_keys() {
        assert!(!fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Pressed,
            false,
            false,
            ModifiersState::empty(),
        ));
        assert!(!fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Pressed,
            true,
            false,
            ModifiersState::ALT,
        ));
        assert!(!fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Released,
            false,
            false,
            ModifiersState::ALT,
        ));
        assert!(!fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::Enter),
            ElementState::Pressed,
            false,
            true,
            ModifiersState::ALT,
        ));
        assert!(!fullscreen_toggle_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::KeyA),
            ElementState::Pressed,
            false,
            false,
            ModifiersState::ALT,
        ));
    }

    #[test]
    fn imgui_visibility_shortcut_accepts_f7_press() {
        assert!(imgui_visibility_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::F7),
            ElementState::Pressed,
            false,
            false,
        ));
    }

    #[test]
    fn imgui_visibility_shortcut_rejects_repeat_release_synthetic_and_other_keys() {
        assert!(!imgui_visibility_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::F7),
            ElementState::Pressed,
            true,
            false,
        ));
        assert!(!imgui_visibility_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::F7),
            ElementState::Released,
            false,
            false,
        ));
        assert!(!imgui_visibility_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::F7),
            ElementState::Pressed,
            false,
            true,
        ));
        assert!(!imgui_visibility_shortcut_from_parts(
            PhysicalKey::Code(KeyCode::F6),
            ElementState::Pressed,
            false,
            false,
        ));
    }
}
