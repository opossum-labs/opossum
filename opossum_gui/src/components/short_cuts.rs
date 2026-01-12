use crate::components::menu_bar::menu_bar_component::AppCommand;
use dioxus::prelude::*;
use std::{collections::HashMap, fmt, path::PathBuf, sync::LazyLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    NewProject,
    OpenProject,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ShortCutAction {
    Center,
    ZoomToFit,
    AutoLayout,
    Save,
    SaveAs,
    Open,
    New,
    Report,
    Simulate,
    Quit,
}

impl fmt::Display for ShortCutAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Center => "Center Graph",
            Self::ZoomToFit => "Zoom to Fit Graph",
            Self::AutoLayout => "Auto Layout",
            Self::Save => "Save",
            Self::SaveAs => "Save As...",
            Self::Open => "Open Project",
            Self::New => "New Project",
            Self::Report => "Set Report Directory",
            Self::Simulate => "Start Simulation",
            Self::Quit => "Quit Application",
        };
        write!(f, "{}", text)
    }
}

impl From<ShortCutAction> for AppCommand {
    fn from(action: ShortCutAction) -> Self {
        match action {
            ShortCutAction::Center => AppCommand::CenterGraph { zoom_to_fit: false },
            ShortCutAction::ZoomToFit => AppCommand::CenterGraph { zoom_to_fit: true },
            ShortCutAction::AutoLayout => AppCommand::AutoLayout,
            // Save vom Shortcut ist immer generisch; App entscheidet ob Save oder SaveAs
            ShortCutAction::Save => AppCommand::Save,
            ShortCutAction::SaveAs => AppCommand::SaveAs,
            ShortCutAction::Open => AppCommand::OpenTrigger,
            ShortCutAction::New => AppCommand::NewProject,
            // Report vom Shortcut triggert Dialog (leerer Pfad als Marker)
            ShortCutAction::Report => AppCommand::SetReportDir(PathBuf::new()),
            ShortCutAction::Simulate => AppCommand::Simulate,
            ShortCutAction::Quit => AppCommand::Quit,
        }
    }
}

pub static SHORTCUTS: LazyLock<HashMap<ShortCutAction, Shortcut>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(ShortCutAction::Open, Shortcut::new(true, false, false, "O", ShortCutAction::Open));
    m.insert(ShortCutAction::Save, Shortcut::new(true, false, false, "S", ShortCutAction::Save));
    m.insert(ShortCutAction::SaveAs, Shortcut::new(true, true, false, "S", ShortCutAction::SaveAs));
    m.insert(ShortCutAction::New, Shortcut::new(true, false, false, "N", ShortCutAction::New));
    m.insert(ShortCutAction::Center, Shortcut::new(true, true, false, "C", ShortCutAction::Center));
    m.insert(ShortCutAction::ZoomToFit, Shortcut::new(true, true, false, "F", ShortCutAction::ZoomToFit));
    m.insert(ShortCutAction::AutoLayout, Shortcut::new(true, true, false, "A", ShortCutAction::AutoLayout));
    m.insert(ShortCutAction::Report, Shortcut::new(true, false, false, "R", ShortCutAction::Report));
    m.insert(ShortCutAction::Simulate, Shortcut::new(false, false, true, "S", ShortCutAction::Simulate));
    m.insert(ShortCutAction::Quit, Shortcut::new(true, false, false, "Q", ShortCutAction::Quit));
    m
});

/// Helfer zum Ermitteln der Action aus einem Event
pub fn get_action_from_event(event: &KeyboardEvent) -> Option<ShortCutAction> {
    SHORTCUTS.values().find(|sc| sc.matches(event)).map(|sc| sc.action)
}

pub const fn primary_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub ctrl_or_meta: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: &'static str,
    pub action: ShortCutAction,
}

impl Shortcut {
    pub fn new(ctrl: bool, shift: bool, alt: bool, key: &'static str, action: ShortCutAction) -> Self {
        Self { ctrl_or_meta: ctrl, shift, alt, key, action }
    }

    pub fn matches(&self, event: &KeyboardEvent) -> bool {
        let modifiers = event.modifiers();
        let key = match event.data().key() {
            Key::Character(s) => s.to_uppercase(),
            _ => return false,
        };
        self.ctrl_or_meta == (modifiers.ctrl() || modifiers.meta())
            && self.shift == modifiers.shift()
            && self.alt == modifiers.alt()
            && self.key.to_uppercase() == key
    }

    pub fn display(&self) -> String {
        let mut parts = vec![];
        if self.ctrl_or_meta { parts.push(primary_modifier_label()); }
        if self.shift { parts.push("Shift"); }
        if self.alt { parts.push("Alt"); }
        parts.push(self.key);
        parts.join("+")
    }
}