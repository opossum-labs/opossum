mod bores;
mod circular_aperture;
mod polygon_aperture;
mod rectangular_aperture;

use crate::components::{
    logger::LogResultExt,
    node_editor::{
        hooks::use_synced_signal,
        inputs::{
            IntoInputData,
            input_components::{LabeledSelect, RowedInputs},
        },
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
    },
};
use bores::BoreEditor;
use circular_aperture::CircularApertureParam;
use dioxus::prelude::*;
use heck::ToLowerCamelCase;
use opossum_core::{
    apertures::{Bore, OpticShape, Outline, OutlineShape},
    prelude::Proptype,
    properties::validator::Validator,
    utils::default_from_name::DefaultFromName,
};
use polygon_aperture::PolygonApertureInput;
use rectangular_aperture::RectApertureParam;
use std::mem::discriminant;
use strum::IntoEnumIterator;
use uuid::Uuid;

/// The kinds of edge the optic shape editor offers: every kind of outline, and `Open` where the
/// validator of the property accepts an open optic shape ([`Validator::may_accept`]).
///
/// # Arguments
///
/// * `current` - the optic shape shown; its kind is the selected option.
/// * `validator` - the validator of the property, if it has one.
///
/// # Returns
///
/// The options of the dropdown, each with whether it is selected and its name.
fn edge_options(current: &OpticShape, validator: Option<&Validator>) -> Vec<(bool, String)> {
    let open = OpticShape::Open;
    let may_be_open =
        validator.is_none_or(|validator| validator.may_accept(&Proptype::OpticShape(open.clone())));
    let kind = match current {
        OpticShape::Open => None,
        OpticShape::Bounded(outline) => Some(discriminant(outline.shape())),
    };
    may_be_open
        .then(|| (kind.is_none(), open.to_string()))
        .into_iter()
        .chain(
            OutlineShape::iter()
                .map(|shape| (kind == Some(discriminant(&shape)), shape.to_string())),
        )
        .collect()
}

/// The inputs for the parameters of an outline: a row of numbers for a circle or a rectangle, the
/// list of points for a polygon.
///
/// # Arguments
///
/// * `shape` - the outline whose parameters are shown.
/// * `on_shape_change` - handler receiving the edited outline.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
fn OutlineShapeInput(
    shape: OutlineShape,
    on_shape_change: EventHandler<OutlineShape>,
    readonly: bool,
) -> Element {
    let inputs = match &shape {
        OutlineShape::Circle(circle) => {
            CircularApertureParam::to_input_data_vec(circle, on_shape_change, readonly)
        }
        OutlineShape::Rectangle(rectangle) => {
            RectApertureParam::to_input_data_vec(rectangle, on_shape_change, readonly)
        }
        OutlineShape::Polygon(polygon) => {
            return rsx! {
                PolygonApertureInput { polygon_config: polygon.clone(), on_shape_change, readonly }
            };
        }
    };
    rsx! {
        RowedInputs { inputs }
    }
}

/// Editor for a node's `optic shape` property: the edge of the component and the bores through
/// it.
///
/// The edge is chosen from the kinds [`edge_options`] offers; choosing another kind starts a fresh
/// edge of that kind without bores, which one undo step reverts. Every change is checked by the
/// core ([`Outline::new`]): one it refuses, such as a bore outside the edge, is logged and not
/// saved.
///
/// # Arguments
///
/// * `node_id` - id of the node whose property is edited.
/// * `optic_shape` - the optic shape to show.
/// * `validator` - the validator of the property, which decides whether `Open` is offered.
/// * `property_key` - name of the edited property, needed for the change event.
/// * `on_change` - handler that carries a property change towards the backend.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn OpticShapeEditor(
    node_id: ReadSignal<Uuid>,
    optic_shape: OpticShape,
    validator: Option<Validator>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let optic_shape_sig = use_synced_signal(optic_shape);
    let on_save =
        on_save_proptype_handler(optic_shape_sig, property_key.clone(), on_change, node_id);
    let on_outline = EventHandler::new(move |(edge, bores): (OutlineShape, Vec<Bore>)| {
        Outline::new(edge, bores)
            .map(|outline| on_save.call(outline.into()))
            .log_err_with_context("the optic shape");
    });

    let current = optic_shape_sig.read().clone();
    let outline_inputs = match &current {
        OpticShape::Open => rsx! {},
        OpticShape::Bounded(outline) => {
            let (edge, bores) = (outline.shape().clone(), outline.bores().to_vec());
            let on_edge_change = EventHandler::new({
                let bores = bores.clone();
                move |edge: OutlineShape| on_outline.call((edge, bores.clone()))
            });
            let on_bores_change = EventHandler::new({
                let edge = edge.clone();
                move |bores: Vec<Bore>| on_outline.call((edge.clone(), bores))
            });
            rsx! {
                OutlineShapeInput { shape: edge, on_shape_change: on_edge_change, readonly }
                BoreEditor { bores, on_bores_change, readonly }
            }
        }
    };

    rsx! {
        LabeledSelect {
            id: format!("opticShapeProperty{property_key}").to_lower_camel_case(),
            label: "Optic shape",
            options: edge_options(&current, validator.as_ref()),
            readonly,
            onchange: move |e: Event<FormData>| {
                let chosen = e.value();
                if chosen == OpticShape::Open.to_string() {
                    on_save.call(OpticShape::Open);
                } else if let Some(edge) = OutlineShape::default_from_name(&chosen) {
                    on_outline.call((edge, Vec::new()));
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start", {outline_inputs} }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::apertures::CircleShape;

    fn names(options: &[(bool, String)]) -> Vec<&str> {
        options.iter().map(|(_, name)| name.as_str()).collect()
    }

    /// Every kind of outline is offered as the edge; `Open` only where the validator accepts it,
    /// which a component's does not and a detector's window, without one, does.
    #[test]
    fn the_validator_decides_whether_open_is_offered() {
        let circle = OpticShape::from(CircleShape::default());
        let component = edge_options(&circle, Some(&Validator::OpticShapeIsBounded));
        assert_eq!(names(&component), ["Circle", "Rectangle", "Polygon"]);
        let window = edge_options(&circle, None);
        assert_eq!(names(&window), ["Open", "Circle", "Rectangle", "Polygon"]);
    }
    /// The kind of the optic shape shown is the selected option.
    #[test]
    fn the_kind_shown_is_selected() {
        let selected = |optic_shape: &OpticShape| {
            edge_options(optic_shape, None)
                .into_iter()
                .filter_map(|(selected, name)| selected.then_some(name))
                .collect::<Vec<_>>()
        };
        assert_eq!(selected(&CircleShape::default().into()), ["Circle"]);
        assert_eq!(selected(&OpticShape::Open), ["Open"]);
    }
}
