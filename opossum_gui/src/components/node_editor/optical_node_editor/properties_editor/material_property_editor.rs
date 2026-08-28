use crate::components::{
    inputs::material_selector::MaterialSelector,
    node_editor::node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::{material::Material, properties::proptype::AssetRef};
use uuid::Uuid;

/// Component for viewing, editing, and assigning optical materials to a node property.
#[component]
pub fn MaterialPropertyEditor(
    /// ID of the node being edited.
    node_id: ReadSignal<Uuid>,
    /// Material reference property of the node.
    material_ref: AssetRef<Material>,
    /// Name/key of the property inside the node.
    property_key: String,
    /// Event handler to propagate changes back to the node graph.
    on_change: EventHandler<NodeChangeEvent>,
    /// Readonly flag to disable editing interactions.
    readonly: bool,
) -> Element {
    let current_material = material_ref.unwrap_inline().clone();
    let prop_key = property_key.clone();

    let on_material_change = move |updated_material: Material| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::Property(
                prop_key.clone(),
                AssetRef::Inline(updated_material).into(),
            ),
        });
    };

    rsx! {
      MaterialSelector {
        label: property_key,
        material: current_material,
        readonly,
        on_change: on_material_change,
      }
    }
}
