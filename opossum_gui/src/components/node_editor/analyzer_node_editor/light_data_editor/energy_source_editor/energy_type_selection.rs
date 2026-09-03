use crate::components::node_editor::analyzer_node_editor::light_data_editor::default_energy_laser_lines;
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
                    let default_wvl = crate::APP_CONFIG.read().default_wavelength();
                    let configured_edb = match edb {
                        EnergyDataBuilder::LaserLines(_) => {
                            EnergyDataBuilder::LaserLines(
                                default_energy_laser_lines(default_wvl),
                            )
                        }
                        other => other,
                    };
                    on_energy_data_builder_save.call(configured_edb);
                }
            },
        }
    }
}
