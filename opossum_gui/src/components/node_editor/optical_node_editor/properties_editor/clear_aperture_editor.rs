use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    node_config_editor::NodeChangeEvent,
    optical_node_editor::{
        port_config_editor::aperture_editor::{
            CircularApertureParam, PolygonApertureInput, RectApertureParam,
        },
        properties_editor::on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{prelude::ApertureShape, utils::default_from_name::DefaultFromName};
use uuid::Uuid;

/// The parameter rows belonging to the currently selected shape.
///
/// Reuses the very same per-shape widgets the port aperture editor is built from — both edit an
/// [`ApertureShape`], so a circle's radius row is written once and shown in both places.
///
/// # Arguments
///
/// * `shape` - the shape whose parameters are shown.
/// * `on_save` - handler receiving the edited shape.
/// * `readonly` - whether the inputs are shown read-only.
///
/// # Returns
///
/// The rows for `shape`, or an empty list for a shape that has no plain numeric parameters (the
/// polygon, which brings its own input component).
fn clear_aperture_input_data(
    shape: &ApertureShape,
    on_save: EventHandler<ApertureShape>,
    readonly: bool,
) -> Vec<InputData> {
    match shape {
        ApertureShape::BinaryCircle(circle) => {
            CircularApertureParam::to_input_data_vec(circle, on_save, readonly)
        }
        ApertureShape::BinaryRectangle(rectangle) => {
            RectApertureParam::to_input_data_vec(rectangle, on_save, readonly)
        }
        _ => Vec::new(),
    }
}

/// Editor for a volume node's `clear aperture` property: a shape selector plus that shape's
/// parameters.
///
/// The dropdown-plus-parameter-rows composition is the one `MaterialEditor` and
/// `RefractiveIndexEditor` use, and the parameter rows themselves are the port aperture editor's.
/// What differs from that editor is the choice offered: it edits a transmission mask and may
/// therefore offer every shape, while this one states the transversal extent of the medium and is
/// restricted to the shapes that actually bound a region (see [`non_delimiting_shapes`]). It also
/// has no aperture *type* and no isometry of its own — the property carries a bare
/// [`ApertureShape`].
///
/// # Arguments
///
/// * `node_id` - id of the node whose property is edited.
/// * `aperture` - the clear aperture shape to show.
/// * `property_key` - name of the edited property, needed for the change event.
/// * `on_change` - handler that carries a property change towards the backend.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn ClearApertureEditor(
    node_id: ReadSignal<Uuid>,
    aperture: ApertureShape,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let aperture_sig = use_synced_signal(aperture);

    let on_save = on_save_proptype_handler(aperture_sig, property_key.clone(), on_change, node_id);

    // Pre-rendered outside the `rsx!` block below, as in the port aperture editor: the polygon
    // brings its own input component rather than a list of numeric rows.
    let current_shape = aperture_sig.read().clone();
    let shape_specific_input = match &current_shape {
        ApertureShape::BinaryPolygon(polygon_config) => rsx! {
            PolygonApertureInput {
                polygon_config: polygon_config.clone(),
                on_shape_change: on_save,
                readonly,
            }
        },
        _ => rsx! {
            RowedInputs { inputs: clear_aperture_input_data(&current_shape, on_save, readonly) }
        },
    };

    // Which shapes can bound a medium is decided by the core, not restated here: the clear aperture
    // states where the material ends, and a shape without an edge cannot say that (the property's
    // validator rejects exactly these).
    let excluded = ApertureShape::non_delimiting();
    let excluded = excluded.iter().collect::<Vec<_>>();
    rsx! {
        LabeledSelect {
            id: format!("clearApertureProperty{property_key}").to_camel_case(),
            label: "Clear aperture",
            options: select_options_from_enum_iterator(&*aperture_sig.read(), Some(&excluded)),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(shape) = ApertureShape::default_from_name(val.as_str()) {
                    on_save.call(shape);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start", {shape_specific_input} }
    }
}
