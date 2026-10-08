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
            CircularApertureParam, PolygonApertureInput, RectApertureParam, StackedApertureInput,
        },
        properties_editor::on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use heck::ToLowerCamelCase;
use opossum_core::{
    apertures::StackShape,
    prelude::{Aperture, ApertureShape, ApertureType, Proptype},
    properties::validator::Validator,
    utils::default_from_name::DefaultFromName,
};
use strum::IntoEnumIterator;
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

/// The shape to switch to when another kind of shape is chosen in the dropdown.
///
/// A new stack starts with the current outline as its hole, so it bounds a region from the start
/// and the bore or notch can be added to it; the stack a bare default would give holds nothing
/// and is refused by the property. Any other shape is switched to as chosen.
///
/// # Arguments
///
/// * `chosen` - the default of the chosen kind of shape.
/// * `current` - the shape the clear aperture has now.
fn shape_to_switch_to(chosen: ApertureShape, current: &ApertureShape) -> ApertureShape {
    if !matches!(chosen, ApertureShape::Stack(_)) {
        return chosen;
    }
    Aperture::new(current.clone(), ApertureType::Hole, None, None)
        .and_then(|outline| StackShape::new(vec![outline]))
        .map_or(chosen, ApertureShape::Stack)
}

/// The kinds of shapes the clear aperture cannot take, as its validator states them.
///
/// The core decides which kinds a property accepts ([`Validator::may_accept`]); a property without
/// a validator accepts every kind.
///
/// # Arguments
///
/// * `validator` - the validator of the clear aperture property, if it has one.
///
/// # Returns
///
/// The [`Default`] of each excluded kind: only which variant each one is carries meaning.
fn excluded_shapes(validator: Option<&Validator>) -> Vec<ApertureShape> {
    ApertureShape::iter()
        .filter(|shape| {
            validator
                .is_some_and(|validator| !validator.may_accept(&Proptype::Aperture(shape.clone())))
        })
        .collect()
}

/// Editor for a node's `clear aperture` property: a shape selector plus that shape's parameters.
///
/// The dropdown-plus-parameter-rows composition is the one `MaterialEditor` and
/// `RefractiveIndexEditor` use, and the parameter rows and the stack editor themselves are the port
/// aperture editor's. What differs from that editor is the choice offered: it edits a transmission
/// mask and may therefore offer every shape, while this one states the transversal extent of the
/// component and offers only the kinds of shapes its validator accepts (see [`excluded_shapes`]):
/// those that can bound a region - a stack, for a ring or a mirror with a central hole, is one of
/// them - and, for a detector's window or an ideal surface, also an open one. It also has no
/// aperture *type* and no isometry of its own — the property carries a bare [`ApertureShape`].
///
/// # Arguments
///
/// * `node_id` - id of the node whose property is edited.
/// * `aperture` - the clear aperture shape to show.
/// * `validator` - the validator of the property, which decides the shapes offered.
/// * `property_key` - name of the edited property, needed for the change event.
/// * `on_change` - handler that carries a property change towards the backend.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn ClearApertureEditor(
    node_id: ReadSignal<Uuid>,
    aperture: ApertureShape,
    validator: Option<Validator>,
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
        ApertureShape::Stack(stack) => rsx! {
            StackedApertureInput {
                stacked_aperture: stack.clone(),
                on_shape_change: on_save,
                readonly,
            }
        },
        _ => rsx! {
            RowedInputs { inputs: clear_aperture_input_data(&current_shape, on_save, readonly) }
        },
    };

    let excluded = excluded_shapes(validator.as_ref());
    let excluded = excluded.iter().collect::<Vec<_>>();
    rsx! {
        LabeledSelect {
            id: format!("clearApertureProperty{property_key}").to_lower_camel_case(),
            label: "Clear aperture",
            options: select_options_from_enum_iterator(&*aperture_sig.read(), Some(&excluded)),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(shape) = ApertureShape::default_from_name(val.as_str()) {
                    on_save.call(shape_to_switch_to(shape, &aperture_sig.read()));
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start", {shape_specific_input} }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn names(shapes: &[ApertureShape]) -> Vec<String> {
        shapes.iter().map(ToString::to_string).collect()
    }

    /// A component must have an edge, so neither an open nor a soft shape is offered; a detector's
    /// window or an ideal surface may also be open; without a validator every shape is offered.
    #[test]
    fn the_validator_decides_which_shapes_are_offered() {
        let open = names(&[ApertureShape::Open]);
        let gaussian = names(&[ApertureShape::Gaussian(Default::default())]);
        let component = names(&excluded_shapes(Some(&Validator::ApertureDelimitsRegion)));
        assert_eq!(component, [open[0].clone(), gaussian[0].clone()]);
        let window = names(&excluded_shapes(Some(
            &Validator::ApertureDelimitsRegionOrIsOpen,
        )));
        assert_eq!(window, gaussian);
        assert!(excluded_shapes(None).is_empty());
    }
}
