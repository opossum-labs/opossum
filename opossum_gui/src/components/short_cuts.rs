use crate::components::menu_bar::{
    menu_bar_component::MenuSelection, save_project, save_project_as, set_report_directory,
};
use dioxus::prelude::*;
use rfd::FileDialog;
use std::{collections::HashMap, sync::LazyLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAction {
    NewProject,
    OpenProject,
    Quit,
}

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
    m.insert(
        ShortCutAction::Simulate,
        Shortcut {
            ctrl_or_meta: false,
            shift: false,
            alt: true,
            key: "S",
            action: ShortCutAction::Simulate,
        },
    );
    m.insert(
        ShortCutAction::Quit,
        Shortcut {
            ctrl_or_meta: true,
            shift: false,
            alt: false,
            key: "Q",
            action: ShortCutAction::Simulate,
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
    Simulate,
    Quit,
}

impl ShortCutAction {
    pub fn run(
        self,
        mut menu_item_selected: Signal<Option<MenuSelection>>,
        model_modified: ReadSignal<bool>,
        model_file_path: ReadSignal<Option<std::path::PathBuf>>,
        mut project_directory: Signal<Option<std::path::PathBuf>>,
        mut pending_action: Signal<Option<PendingAction>>,
        mut show_alert: Signal<bool>,
    ) {
        match self {
            Self::Center => {
                menu_item_selected.set(Some(MenuSelection::CenterGraph { zoom_to_fit: false }));
            }
            Self::ZoomToFit => {
                menu_item_selected.set(Some(MenuSelection::CenterGraph { zoom_to_fit: true }));
            }
            Self::AutoLayout => menu_item_selected.set(Some(MenuSelection::AutoLayout)),
            Self::Save => {
                spawn(async move {
                    save_project(model_file_path, menu_item_selected).await;
                });
            }
            Self::SaveAs => {
                spawn(async move {
                    save_project_as(menu_item_selected).await;
                });
            }
            Self::Open => {
                if *model_modified.read() {
                    pending_action.set(Some(PendingAction::OpenProject));
                    show_alert.set(true);
                } else {
                    // --- ÄNDERUNG ---
                    spawn(async move {
                        crate::components::menu_bar::project_helper::open_project(
                            menu_item_selected,
                        )
                        .await;
                    });
                }
            }
            Self::New => {
                if *model_modified.read() {
                    pending_action.set(Some(PendingAction::NewProject));
                    show_alert.set(true);
                } else {
                    menu_item_selected.set(Some(MenuSelection::NewProject));
                }
            }
            Self::Report => {
                spawn(async move {
                    set_report_directory(menu_item_selected).await;
                });
            }
            Self::Simulate => {if project_directory().is_none() {
                                    let path = FileDialog::new()
                                        .set_directory("./")
                                        .set_title("Select OPOSSUM report directory")
                                        .pick_folder();
                                    if let Some(path) = path {
                                        project_directory.set(Some(path));
                                        menu_item_selected.set(Some(MenuSelection::RunProject));
                                    }
                                } else {
                                    menu_item_selected.set(Some(MenuSelection::RunProject));
                                }}
            Self::Quit => {}
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
            Self::Simulate => "Start Simulation",
            Self::Quit => "Quit Application",
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
    model_modified: ReadSignal<bool>,
    model_file_path: ReadSignal<Option<std::path::PathBuf>>,
    project_directory: Signal<Option<std::path::PathBuf>>,
    pending_action: Signal<Option<PendingAction>>,
    show_alert: Signal<bool>,
}

impl ShortcutHandler {
    pub const fn new(
        menu_item_selected: Signal<Option<MenuSelection>>,
        model_modified: ReadSignal<bool>,
        model_file_path: ReadSignal<Option<std::path::PathBuf>>,
        project_directory: Signal<Option<std::path::PathBuf>>,
        pending_action: Signal<Option<PendingAction>>,
        show_alert: Signal<bool>,
    ) -> Self {
        Self {
            menu_item_selected,
            model_modified,
            model_file_path,
            project_directory,
            pending_action,
            show_alert,
        }
    }

    /// Handler für Tastatur-Events
    pub fn handle_event(&self, event: &KeyboardEvent) {
        if let Some(sc) = SHORTCUTS.values().find(|sc| sc.matches(event)) {
            sc.action.run(
                self.menu_item_selected,
                self.model_modified,
                self.model_file_path,
                self.project_directory,
                self.pending_action,
                self.show_alert,
            );
        }
    }

    /// Emulator für Klicks (z. B. Button-Handler)
    pub fn emulate(&self, action: ShortCutAction) {
        action.run(
            self.menu_item_selected,
            self.model_modified,
            self.model_file_path,
            self.project_directory,
            self.pending_action,
            self.show_alert,
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
