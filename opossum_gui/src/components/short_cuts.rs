use crate::components::menu_bar::menu_bar_component::AppCommand;
use dioxus::prelude::*;
use std::{collections::HashMap, fmt, sync::LazyLock};

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
    Simulate,
    Settings,
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
            Self::Simulate => "Start Simulation",
            Self::Settings => "Settings",
            Self::Quit => "Quit",
        };
        write!(f, "{text}")
    }
}

impl From<ShortCutAction> for AppCommand {
    fn from(action: ShortCutAction) -> Self {
        match action {
            ShortCutAction::Center => Self::CenterGraph,
            ShortCutAction::ZoomToFit => Self::ZoomToFit,
            ShortCutAction::AutoLayout => Self::AutoLayout,
            ShortCutAction::Save => Self::Save,
            ShortCutAction::SaveAs => Self::SaveAs,
            ShortCutAction::Open => Self::OpenTrigger,
            ShortCutAction::New => Self::NewProject,
            ShortCutAction::Simulate => Self::Simulate,
            ShortCutAction::Settings => Self::Settings,
            ShortCutAction::Quit => Self::Quit,
        }
    }
}

pub static SHORTCUTS: LazyLock<HashMap<ShortCutAction, Shortcut>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(
        ShortCutAction::Open,
        Shortcut::new(true, false, false, "O", ShortCutAction::Open),
    );
    m.insert(
        ShortCutAction::Save,
        Shortcut::new(true, false, false, "S", ShortCutAction::Save),
    );
    m.insert(
        ShortCutAction::SaveAs,
        Shortcut::new(true, true, false, "S", ShortCutAction::SaveAs),
    );
    m.insert(
        ShortCutAction::New,
        Shortcut::new(true, false, false, "N", ShortCutAction::New),
    );
    m.insert(
        ShortCutAction::Center,
        Shortcut::new(true, true, false, "C", ShortCutAction::Center),
    );
    m.insert(
        ShortCutAction::ZoomToFit,
        Shortcut::new(true, true, false, "F", ShortCutAction::ZoomToFit),
    );
    m.insert(
        ShortCutAction::AutoLayout,
        Shortcut::new(true, true, false, "A", ShortCutAction::AutoLayout),
    );
    m.insert(
        ShortCutAction::Simulate,
        Shortcut::new(false, false, true, "S", ShortCutAction::Simulate),
    );
    m.insert(
        ShortCutAction::Settings,
        Shortcut::new(true, false, true, ",", ShortCutAction::Settings),
    );
    m.insert(
        ShortCutAction::Quit,
        Shortcut::new(true, false, false, "Q", ShortCutAction::Quit),
    );
    m
});

pub fn get_action_from_event(event: &KeyboardEvent) -> Option<ShortCutAction> {
    SHORTCUTS
        .values()
        .find(|sc| sc.matches(event))
        .map(|sc| sc.action)
}

pub const fn primary_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl"
    }
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
    pub const fn new(
        ctrl: bool,
        shift: bool,
        alt: bool,
        key: &'static str,
        action: ShortCutAction,
    ) -> Self {
        Self {
            ctrl_or_meta: ctrl,
            shift,
            alt,
            key,
            action,
        }
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
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = vec![];
        if self.ctrl_or_meta {
            parts.push(primary_modifier_label());
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.push(self.key);
        write!(f, "{}", parts.join("+"))
    }
}
