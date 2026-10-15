//! Shared ImGui table helpers for debug HUD windows.

use imgui::TableFlags;

/// Standard scrollable diagnostics table flags.
pub(super) fn scrolling_table_flags() -> TableFlags {
    TableFlags::BORDERS
        | TableFlags::ROW_BG
        | TableFlags::SCROLL_Y
        | TableFlags::RESIZABLE
        | TableFlags::SIZING_STRETCH_PROP
}
