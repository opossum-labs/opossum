use std::fmt::Display;

use crate::components::menu_bar::sub_menu_item::MenuItem;
use dioxus::prelude::*;
use uuid::Uuid;

#[component]
pub fn NodeDropDownMenu<T: 'static + Clone + PartialEq + Display>(
    title: &'static str,
    element_list: Vec<T>,
    on_element_click: fn(T, Uuid) -> Callback<Event<MouseData>>,
) -> Element {
    let current_group = use_context::<Signal<Uuid>>();
    rsx! {
        a { class: "title-bar-submenu-item expand",
            {title}
            div {
                class: "dropdown-content submenu title-bar-dropdown-content",
                id: "node_drop_down_menu",
                for element in element_list.iter() {
                    {
                        rsx! {
                            MenuItem {
                                class: "title-bar-submenu-item".to_owned(),
                                onclick: Some(on_element_click(element.clone(), current_group())),
                                display: format!("{}", element),
                            }
                        }
                    }
                }
            }
        }
    }
}
