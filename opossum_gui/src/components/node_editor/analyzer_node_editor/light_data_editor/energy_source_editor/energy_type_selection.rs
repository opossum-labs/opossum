use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use opossum_core::{
    light::Spectrum, prelude::EnergyDataBuilder, utils::default_from_name::DefaultFromName,
};

#[component]
pub fn EnergyDataBuilderSelector(
    energy_data_builder_sig: ReadSignal<EnergyDataBuilder>,
    on_energy_data_builder_save: EventHandler<EnergyDataBuilder>,
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectEnergyDataType",
            label: "Energy Type",
            options: select_options_from_enum_iterator(
                &*energy_data_builder_sig.read(),
                Some(&[&EnergyDataBuilder::Raw(Spectrum::default())]),
            ),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(edb) = EnergyDataBuilder::default_from_name(val.as_str()) {
                    on_energy_data_builder_save.call(edb);
                }
            },
        }
    }
}
