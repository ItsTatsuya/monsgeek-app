//! Keyboard commands for the whole window, and their shortcuts.

use gpui::{App, KeyBinding};

gpui::actions!(
    monsgeek,
    [
        Undo,
        Redo,
        SelectAllKeys,
        ClearSelection,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        GoPerformance,
        GoRemap,
        GoAdvanced,
        GoLighting,
        GoTester,
        GoAutoProfiles,
        GoSettings,
        Profile1,
        Profile2,
        Profile3,
        Profile4,
    ]
);

/// The key context set on the app's root element.
pub const CONTEXT: &str = "MonsgeekApp";

pub fn bind(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("ctrl-z", Undo, c),
        KeyBinding::new("ctrl-y", Redo, c),
        KeyBinding::new("ctrl-shift-z", Redo, c),
        KeyBinding::new("ctrl-a", SelectAllKeys, c),
        KeyBinding::new("escape", ClearSelection, c),
        KeyBinding::new("ctrl-=", ZoomIn, c),
        KeyBinding::new("ctrl-+", ZoomIn, c),
        KeyBinding::new("ctrl--", ZoomOut, c),
        KeyBinding::new("ctrl-0", ZoomReset, c),
        KeyBinding::new("ctrl-1", GoPerformance, c),
        KeyBinding::new("ctrl-2", GoRemap, c),
        KeyBinding::new("ctrl-3", GoAdvanced, c),
        KeyBinding::new("ctrl-4", GoLighting, c),
        KeyBinding::new("ctrl-5", GoTester, c),
        KeyBinding::new("ctrl-6", GoAutoProfiles, c),
        KeyBinding::new("ctrl-7", GoSettings, c),
        KeyBinding::new("alt-1", Profile1, c),
        KeyBinding::new("alt-2", Profile2, c),
        KeyBinding::new("alt-3", Profile3, c),
        KeyBinding::new("alt-4", Profile4, c),
    ]);
}
