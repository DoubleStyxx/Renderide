//! IME enable request construction for the renderer window.

use winit::dpi::{LogicalPosition, LogicalSize};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData, Window,
};

/// Enables IME text input with the renderer's default hint and cursor-area capabilities.
pub fn enable_ime_on_window(window: &dyn Window) {
    let Some(request) = default_ime_enable_request() else {
        logger::warn!("IME enable request could not be built from default request data");
        return;
    };
    let _ = window.request_ime_update(ImeRequest::Enable(request));
}

pub(super) fn default_ime_enable_request() -> Option<ImeEnableRequest> {
    let position = LogicalPosition::new(0, 0);
    let size = LogicalSize::new(0, 0);
    let ime_caps = ImeCapabilities::new()
        .with_hint_and_purpose()
        .with_cursor_area();
    let request_data = ImeRequestData::default()
        .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
        .with_cursor_area(position.into(), size.into());
    ImeEnableRequest::new(ime_caps, request_data)
}

#[cfg(test)]
mod tests {
    use super::default_ime_enable_request;

    #[test]
    fn default_ime_enable_request_matches_capabilities() {
        let request = default_ime_enable_request().expect("request data matches capabilities");
        assert!(request.capabilities().hint_and_purpose());
        assert!(request.capabilities().cursor_area());
        assert!(!request.capabilities().surrounding_text());
        assert!(request.request_data().hint_and_purpose.is_some());
        assert!(request.request_data().cursor_area.is_some());
        assert!(request.request_data().surrounding_text.is_none());
    }
}
