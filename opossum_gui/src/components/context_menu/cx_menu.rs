use crate::{components::menu_bar::sub_menu_item::MenuItem, MAIN_WINDOW_SIZE};
use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct CxMenu {
    pub x: f64,
    pub y: f64,
    pub entries: Vec<(String, Callback<Event<MouseData>>)>,
}
impl CxMenu {
    pub fn height(num_entries: usize) -> f64 {
        22.4 * num_entries as f64 + 2. * Self::padding()
    }

    pub fn width() -> f64 {
        150. + 2. * Self::padding()
    }
    pub fn padding() -> f64 {
        2.
    }

    pub fn entries(&self) -> &Vec<(String, Callback<Event<MouseData>>)> {
        &self.entries
    }

    pub fn set(&mut self, cx_menu: Self) {
        *self = cx_menu;
    }

    pub fn new(x: f64, y: f64, entries: Vec<(String, Callback<Event<MouseData>>)>) -> Option<Self> {
        if let Some(rect) = MAIN_WINDOW_SIZE.read().as_ref() {
            let mut x = x;
            let mut y = y;
            if x + Self::width() > rect.width {
                x -= Self::width()
            }
            if y + Self::height(entries.len()) > rect.height {
                y -= Self::height(entries.len())
            }
            Some(Self { x, y, entries })
        } else {
            None
        }
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
