use crate::{components::menu_bar::sub_menu_item::MenuItem, MAIN_WINDOW_SIZE};
use dioxus::prelude::*;
use opossum_backend::usize_to_f64;

#[derive(Clone, PartialEq)]
pub struct CxMenu {
    pub x: f64,
    pub y: f64,
    pub entries: Vec<(String, Callback<Event<MouseData>>)>,
}
impl CxMenu {
    #[must_use]
    pub const fn height(num_entries: usize) -> f64 {
        22.4 * usize_to_f64(num_entries) + 2. * Self::padding()
    }
    #[must_use]
    pub const fn width() -> f64 {
        150. + 2. * Self::padding()
    }
    #[must_use]
    pub const fn padding() -> f64 {
        2.
    }
    #[must_use]
    pub const fn entries(&self) -> &Vec<(String, Callback<Event<MouseData>>)> {
        &self.entries
    }

    pub fn set(&mut self, cx_menu: Self) {
        *self = cx_menu;
    }
    #[must_use]
    pub fn new(x: f64, y: f64, entries: Vec<(String, Callback<Event<MouseData>>)>) -> Option<Self> {
        MAIN_WINDOW_SIZE.read().as_ref().map(|rect| {
            let mut x = x;
            let mut y = y;
            if x + Self::width() > rect.width {
                x -= Self::width();
            }
            if y + Self::height(entries.len()) > rect.height {
                y -= Self::height(entries.len());
            }
            Self { x, y, entries }
        })
    }
}

#[component]
pub fn ContextMenu(cx_menu: Option<CxMenu>) -> Element {
    if let Some(cx_menu) = cx_menu {
        let (x, y) = (cx_menu.x, cx_menu.y);
        let width = CxMenu::width();
        let padding = CxMenu::padding();
        rsx!(
            div {
                id: "context-menu",
                style: "top: {y}px; left: {x}px; width:{width}px; padding:{padding}px",

                for (element , on_element_click) in cx_menu.entries.iter() {
                    {
                        rsx! {
                            MenuItem {
                                class: "context-menu-item".to_owned(),
                                onclick: Some(*on_element_click),
                                display: format!("{}", element),
                            }
                        }
                    }
                }
            }
        )
    } else {
        rsx! {
            div { id: "context-menu" }
        }
    }
}
