#![allow(clippy::derive_partial_eq_without_eq)]

mod fibonacci_ellipse_editor;
mod fibonacci_rectangle_editor;
mod grid_editor;
mod hexagonal_editor;
mod hexapolar_editor;
mod random_editor;
mod sobol_editor;

use fibonacci_ellipse_editor::FibonacciEllipseParam;
use fibonacci_rectangle_editor::FibonacciRectParam;
use grid_editor::GridParam;
use hexagonal_editor::get_hexagonal_input_params;
use hexapolar_editor::get_hexapolar_input_params;
use opossum_core::{
    distributions::position::PosDistType, prelude::RayDataSource,
    utils::default_from_name::DefaultFromName,
};
use random_editor::RandomParam;
use sobol_editor::SobolParam;

use crate::components::node_editor::{
    accordion::AccordionItem,
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
};
use dioxus::prelude::*;
fn get_pos_dist_input_data(
    pos_dist_type_sig: ReadSignal<PosDistType>,
    on_pos_dist_save: EventHandler<PosDistType>,
    readonly: bool,
) -> Vec<InputData> {
    match &*pos_dist_type_sig.read() {
        PosDistType::Random(r) => RandomParam::to_input_data_vec(r, on_pos_dist_save, readonly),
        PosDistType::Sobol(s) => SobolParam::to_input_data_vec(s, on_pos_dist_save, readonly),
        PosDistType::Grid(g) => GridParam::to_input_data_vec(g, on_pos_dist_save, readonly),
        PosDistType::HexagonalTiling(h) => {
            get_hexagonal_input_params(h, on_pos_dist_save, readonly)
        }
        PosDistType::Hexapolar(h) => get_hexapolar_input_params(h, on_pos_dist_save, readonly),
        PosDistType::FibonacciRectangle(f) => {
            FibonacciRectParam::to_input_data_vec(f, on_pos_dist_save, readonly)
        }
        PosDistType::FibonacciEllipse(f) => {
            FibonacciEllipseParam::to_input_data_vec(f, on_pos_dist_save, readonly)
        }
    }
}

#[component]
pub fn RayPositionDistributionSelector(
    pos_dist_type_sig: Signal<PosDistType>,
    on_pos_dist_save: EventHandler<PosDistType>,
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaysPosDistribution",
            label: "Rays Position Distribution",
            options: select_options_from_enum_iterator(&*pos_dist_type_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(pdt) = PosDistType::default_from_name(val.as_str()) {
                    on_pos_dist_save.call(pdt);
                }
            },
        }
    }
}

#[component]
pub fn NodePosDistInputs(
    pos_dist_type_sig: ReadSignal<PosDistType>,
    on_pos_dist_save: EventHandler<PosDistType>,
    readonly: bool,
) -> Element {
    let inputs: Vec<InputData> =
        get_pos_dist_input_data(pos_dist_type_sig, on_pos_dist_save, readonly);
    rsx! {
        RowedInputs { inputs }
    }
}

#[component]
pub fn PositionDistributionEditor(
    pos_dist_type: PosDistType,
    ray_data_builder_sig: ReadSignal<RayDataSource>,
    on_save: EventHandler<RayDataSource>,
    readonly: bool,
) -> Element {
    let mut pos_dist_type_sig = use_signal(|| pos_dist_type);

    let on_pos_dist_save = EventHandler::new(move |new_pos_dist_type: PosDistType| {
        pos_dist_type_sig.set(new_pos_dist_type);
        let mut ray_data_builder = ray_data_builder_sig.read().clone();
        ray_data_builder.set_pos_dist(*pos_dist_type_sig.read());
        on_save.call(ray_data_builder);
    });

    let accordion_item_content = rsx! {
        RayPositionDistributionSelector { pos_dist_type_sig, on_pos_dist_save, readonly }
        NodePosDistInputs { pos_dist_type_sig, on_pos_dist_save, readonly }
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
