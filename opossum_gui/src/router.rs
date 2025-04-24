#[rustfmt::skip]
use crate::components::{
    app::App,
    menu_bar::{help::about::About, menu_bar_component::MenuBar},
    page_not_found::PageNotFound,
};
use dioxus::prelude::*;

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(MenuBar)]
        #[route("/")]
        App,
        #[nest("/about")]
            #[route("/")]
             About,
        #[end_nest]
    #[end_layout]
    #[route("/:..route")]
    PageNotFound { route: Vec<String> },
}
