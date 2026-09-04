use crate::components::node_editor::{
    analyzer_node_editor::light_data_editor::apply_default_wavelength_to_ray_source,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
};
use dioxus::prelude::*;
use opossum_core::{
    light::Rays, prelude::RayDataSource, utils::default_from_name::DefaultFromName,
};

#[component]
pub fn RayDataBuilderSelector(
    ray_data_builder_sig: ReadSignal<RayDataSource>,
    on_save: EventHandler<RayDataSource>,
    readonly: bool,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaySourceType",
            label: "Rays Type",
            options: select_options_from_enum_iterator(
                &*ray_data_builder_sig.read(),
                Some(&[&RayDataSource::Raw(Rays::default())]),
            ),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(mut rdb) = RayDataSource::default_from_name(val.as_str()) {
                    let default_wvl = crate::APP_CONFIG.read().default_wavelength();
                    apply_default_wavelength_to_ray_source(&mut rdb, default_wvl);
                    on_save.call(rdb);
                }
            },
        }
    }
}
