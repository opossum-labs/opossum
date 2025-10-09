use crate::components::menu_bar::{
    menu_bar_component::MenuSelection, new_project, open_project, save_project, save_project_as,
    set_report_directory,
};
use dioxus::prelude::*;
use std::{collections::HashMap, sync::LazyLock};

pub static SHORTCUTS: LazyLock<HashMap<ShortCutAction, Shortcut>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(
        ShortCutAction::Open,
        Shortcut {
            ctrl_or_meta: true,
            shift: false,
            alt: false,
            key: "O",
            action: ShortCutAction::Open,
        },
    );
    m.insert(
        ShortCutAction::Save,
        Shortcut {
            ctrl_or_meta: true,
            shift: false,
            alt: false,
            key: "S",
            action: ShortCutAction::Save,
        },
    );
    m.insert(
        ShortCutAction::SaveAs,
        Shortcut {
            ctrl_or_meta: true,
            shift: true,
            alt: false,
            key: "S",
            action: ShortCutAction::SaveAs,
        },
    );
    m.insert(
        ShortCutAction::New,
        Shortcut {
            ctrl_or_meta: true,
            shift: false,
            alt: false,
            key: "N",
            action: ShortCutAction::New,
        },
    );
    m.insert(
        ShortCutAction::Center,
        Shortcut {
            ctrl_or_meta: true,
            shift: true,
            alt: false,
            key: "C",
            action: ShortCutAction::Center,
        },
    );
    m.insert(
        ShortCutAction::ZoomToFit,
        Shortcut {
            ctrl_or_meta: true,
            shift: true,
            alt: false,
            key: "F",
            action: ShortCutAction::ZoomToFit,
        },
    );
    m.insert(
        ShortCutAction::AutoLayout,
        Shortcut {
            ctrl_or_meta: true,
            shift: true,
            alt: false,
            key: "A",
            action: ShortCutAction::AutoLayout,
        },
    );
    m.insert(
        ShortCutAction::Report,
        Shortcut {
            ctrl_or_meta: true,
            shift: false,
            alt: false,
            key: "R",
            action: ShortCutAction::Report,
        },
    );
    m
});

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
}

impl ShortCutAction {
    pub fn run(
        self,
        mut menu_item_selected: Signal<Option<MenuSelection>>,
        model_modified: Signal<bool>,
        model_file_path: Signal<Option<std::path::PathBuf>>,
    ) {
        match self {
            Self::Center => {
                menu_item_selected.set(Some(MenuSelection::CenterGraph { zoom_to_fit: false }));
            }
            Self::ZoomToFit => {
                menu_item_selected.set(Some(MenuSelection::CenterGraph { zoom_to_fit: true }));
            }
            Self::AutoLayout => menu_item_selected.set(Some(MenuSelection::AutoLayout)),
            Self::Save => save_project(model_file_path, menu_item_selected),
            Self::SaveAs => save_project_as(menu_item_selected),
            Self::Open => open_project(menu_item_selected, model_modified),
            Self::New => new_project(menu_item_selected, model_modified),
            Self::Report => set_report_directory(menu_item_selected),
        }
    }
    pub const fn display(self) -> &'static str {
        match self {
            Self::Center => "Center Graph",
            Self::ZoomToFit => "Zoom to Fit Graph",
            Self::AutoLayout => "Auto Layout",
            Self::Save => "Save",
            Self::SaveAs => "Save As...",
            Self::Open => "Open Project",
            Self::New => "New Project",
            Self::Report => "Set Report Directory",
        }
    }
}

pub const fn primary_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘" // Command-Key Symbol
    } else {
        "Ctrl"
    }
}

#[derive(Clone, Copy)]
pub struct ShortcutHandler {
    menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
    model_file_path: Signal<Option<std::path::PathBuf>>,
}

impl ShortcutHandler {
    pub const fn new(
        menu_item_selected: Signal<Option<MenuSelection>>,
        model_modified: Signal<bool>,
        model_file_path: Signal<Option<std::path::PathBuf>>,
    ) -> Self {
        Self {
            menu_item_selected,
            model_modified,
            model_file_path,
        }
    }

    /// Handler für Tastatur-Events
    pub fn handle_event(&self, event: &KeyboardEvent) {
        if let Some(sc) = SHORTCUTS.values().find(|sc| sc.matches(event)) {
            sc.action.run(
                self.menu_item_selected,
                self.model_modified,
                self.model_file_path,
            );
        }
    }

    /// Emulator für Klicks (z. B. Button-Handler)
    pub fn emulate(&self, action: ShortCutAction) {
        action.run(
            self.menu_item_selected,
            self.model_modified,
            self.model_file_path,
        );
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

        parts.join("+")
    }
}
