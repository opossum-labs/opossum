use crate::components::node_editor::{
    inputs::input_components::{NodeConfigUnitInput, UnitHandling},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::num_per_m;
use uom::si::f64::LinearNumberDensity;
use uuid::Uuid;

#[component]
pub fn LinearDensityEditor(
    node_id: Memo<Uuid>,
    linear_density: LinearNumberDensity,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let linear_density_sig = use_signal(|| linear_density);

    let on_save = on_save_proptype_handler(
        linear_density_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    rsx! {
        NodeConfigUnitInput {
            id: format!("linearDensityProperty{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            value: linear_density_sig.read().value,
            unit_config: UnitHandling::new("m⁻¹", true),
            reciprocal: true,
            readonly,
            onchange: move |new_linear_density: f64| {
                on_save.call(num_per_m!(new_linear_density));
            },
        }
    }
}
