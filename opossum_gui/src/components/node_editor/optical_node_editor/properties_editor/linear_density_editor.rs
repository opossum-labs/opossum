use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop, inputs::input_components::{LabeledInput, NodeConfigUnitInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{num_per_m, num_per_mm};
use uom::si::{f64::LinearNumberDensity, linear_number_density::per_millimeter};
use uuid::Uuid;

#[component]
pub fn LinearDensityEditor(
    node_id: Uuid,
    linear_density: LinearNumberDensity,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut linear_density_sig = use_signal(|| linear_density);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        linear_density,
        linear_density_sig,
        on_change,
    );
    rsx! {
        // NodeConfigUnitInput {
        //     id: format!("linearDensityProperty{property_key}").to_camel_case(),
        //     label: property_key.to_sentence_case(),
        //     value: linear_density_sig.read().value,
        //     base_unit: "m⁻¹",
        //     onchange: move |new_linear_density: f64| {
        //         if relative_ne!(linear_density.value, new_linear_density) {
        //             linear_density_sig.set(num_per_m!(new_linear_density));
        //             on_change
        //                 .call(NodeChangeEvent {
        //                     node_id,
        //                     action: NodeChangeAction::Property(
        //                         property_key.clone(),
        //                         num_per_m!(new_linear_density).into(),
        //                     ),
        //                 });
        //         }
        //     },
        // }
        LabeledInput {
            id: format!("linearDensityProperty{property_key}").to_camel_case(),
            label: format!("{} in 1/mm", property_key.to_sentence_case()),
            value: format!("{:.3}", linear_density_sig.read().get::<per_millimeter>()),
            r#type: "number",
            onchange: move |e: Event<FormData>| {
                if let Ok(linear_density) = e.data.value().parse::<f64>() {
                    linear_density_sig.set(num_per_mm!(linear_density));
                }
            },
        }
    }
}
