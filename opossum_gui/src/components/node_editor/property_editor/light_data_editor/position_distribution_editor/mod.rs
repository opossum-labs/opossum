#![allow(clippy::derive_partial_eq_without_eq)]

mod fibonacci_ellipse_editor;
mod fibonacci_rectangle_editor;
mod grid_editor;
mod hexagonal_editor;
mod hexapolar_editor;
mod random_editor;
mod sobol_editor;

pub use fibonacci_ellipse_editor::FibonacciEllipseParam;
pub use fibonacci_rectangle_editor::FibonacciRectParam;
pub use grid_editor::GridParam;
pub use random_editor::RandomParam;
pub use sobol_editor::SobolParam;

use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::{
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator, InputData, IntoInputData,
    },
    property_editor::light_data_editor::position_distribution_editor::{
        hexagonal_editor::get_hexagonal_input_params, hexapolar_editor::get_hexapolar_input_params,
    },
};
use dioxus::prelude::*;
use opossum_backend::{DefaultFromName, PosDistType};
fn get_pos_dist_input_data(
    pos_dist_type: PosDistType,
    pos_dist_type_sig: Signal<PosDistType>,
) -> Vec<InputData> {
    match &pos_dist_type {
        PosDistType::Random(r) => RandomParam::to_input_data_vec(r, pos_dist_type_sig),
        PosDistType::Sobol(s) => SobolParam::to_input_data_vec(s, pos_dist_type_sig),
        PosDistType::Grid(g) => GridParam::to_input_data_vec(g, pos_dist_type_sig),
        PosDistType::HexagonalTiling(h) => get_hexagonal_input_params(h, pos_dist_type_sig),
        PosDistType::Hexapolar(h) => get_hexapolar_input_params(h, pos_dist_type_sig),
        PosDistType::FibonacciRectangle(f) => {
            FibonacciRectParam::to_input_data_vec(f, pos_dist_type_sig)
        }
        PosDistType::FibonacciEllipse(f) => {
            FibonacciEllipseParam::to_input_data_vec(f, pos_dist_type_sig)
        }
    }
}

#[component]
pub fn RayPositionDistributionSelector(pos_dist_type_sig: Signal<PosDistType>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysPosDistribution",
            label: "Rays Position Distribution",
            options: select_options_from_enum_iterator(
                &*pos_dist_type_sig.read(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(pdt) = PosDistType::default_from_name(val.as_str()) {
                    pos_dist_type_sig.set(pdt);
                }
            },
        }
    }
}

#[component]
pub fn NodePosDistInputs(pos_dist_type_sig: Signal<PosDistType>) -> Element {
    let inputs: Vec<InputData> = get_pos_dist_input_data(pos_dist_type_sig(), pos_dist_type_sig);
    rsx! {
        RowedInputs { inputs }
    }
}

#[component]
pub fn PositionDistributionEditor(pos_dist_type_sig: Signal<PosDistType>) -> Element {
    let accordion_item_content = rsx! {
        RayPositionDistributionSelector { pos_dist_type_sig }
        NodePosDistInputs { pos_dist_type_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Position Distribution",
            header_id: "sourcePositionDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourcePositionDistCollapse",
        }
    }
}
