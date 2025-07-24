#![allow(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;
use opossum_backend::{energy_data_builder::EnergyDataBuilder, DefaultFromName, Spectrum};

use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};

#[component]
pub fn EnergyDataBuilderSelector(energy_data_builder_sig: Signal<EnergyDataBuilder>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectEnergyDataType",
            label: "Energy Type",
            options: select_options_from_enum_iterator(
                &*energy_data_builder_sig.read(),
                Some(&[&EnergyDataBuilder::Raw(Spectrum::default())]),
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(edb) = EnergyDataBuilder::default_from_name(val.as_str()) {
                    energy_data_builder_sig.set(edb);
                }
            },
        }
    }
}
