use super::OutlineShapeInput;
use crate::components::node_editor::inputs::{
    InputParam, IntoInputData, IntoInputDataStrings,
    dynamic_list::DynamicListComponent,
    format_si_with_base_unit,
    input_components::{LabeledSelect, RowedInputs, UnitHandling},
    select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use nalgebra::Point2;
use opossum_core::{
    apertures::{Bore, CircleShape, OutlineShape},
    degree, meter,
    utils::default_from_name::DefaultFromName,
};
use strum::EnumIter;
use uom::si::angle::degree;

/// Where a bore lies: its offset from the axis and its rotation about its center.
#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
enum BoreParam {
    X,
    Y,
    Rotation,
}

impl From<BoreParam> for InputParam {
    fn from(value: BoreParam) -> Self {
        match value {
            BoreParam::X => Self::SIUnit("Offset x".into(), "m".into()),
            BoreParam::Y => Self::SIUnit("Offset y".into(), "m".into()),
            BoreParam::Rotation => Self::SIUnit("Rotation".into(), "°".into()),
        }
    }
}

impl IntoInputDataStrings<Bore> for BoreParam {
    fn create_id_string(&self) -> String {
        format!("bore{self:?}Input")
    }

    fn create_value_string(&self, bore: &Bore) -> String {
        match self {
            Self::X => format!("{}", bore.center().x.value),
            Self::Y => format!("{}", bore.center().y.value),
            Self::Rotation => format!("{}", bore.rotation().get::<degree>()),
        }
    }
}

impl IntoInputData<f64, Bore, Bore> for BoreParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut Bore, f64) {
        let param = *self;
        move |bore: &mut Bore, value: f64| {
            let (mut center, mut rotation) = (bore.center(), bore.rotation());
            match param {
                Self::X => center.x = meter!(value),
                Self::Y => center.y = meter!(value),
                Self::Rotation => rotation = degree!(value),
            }
            *bore = Bore::new(bore.shape().clone(), center, rotation);
        }
    }
}

/// The columns a bore is listed with: its shape, its offset and its rotation.
fn list_entry(bore: &Bore) -> Vec<String> {
    let length = |value: f64| format_si_with_base_unit(value, &UnitHandling::new("m", true), false);
    vec![
        bore.shape().to_string(),
        format!("x: {}", length(bore.center().x.value)),
        format!("y: {}", length(bore.center().y.value)),
        format!("{}°", bore.rotation().get::<degree>()),
    ]
}

/// Editor for the bores through a component: a bore being drafted, with its shape, offset and
/// rotation, and the list of the bores the optic shape has.
///
/// Adding, updating and deleting a bore each hands the whole list on once, so each is one step to
/// undo. Editing the draft hands nothing on.
///
/// # Arguments
///
/// * `bores` - the bores the optic shape has.
/// * `on_bores_change` - handler receiving the changed list of bores.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn BoreEditor(
    bores: Vec<Bore>,
    on_bores_change: EventHandler<Vec<Bore>>,
    readonly: bool,
) -> Element {
    let mut draft =
        use_signal(|| Bore::new(CircleShape::default(), Point2::origin(), degree!(0.0)));
    let mut editing_index = use_signal(|| None::<usize>);

    let on_draft_change = EventHandler::new(move |bore: Bore| draft.set(bore));
    let on_draft_shape_change = EventHandler::new(move |shape: OutlineShape| {
        let (center, rotation) = (draft.read().center(), draft.read().rotation());
        draft.set(Bore::new(shape, center, rotation));
    });
    let delete_entry_handler = EventHandler::new({
        let bores = bores.clone();
        move |index: usize| {
            let mut bores = bores.clone();
            if index < bores.len() {
                bores.remove(index);
                on_bores_change.call(bores);
            }
        }
    });
    let modify_entry_handler = EventHandler::new({
        let bores = bores.clone();
        move |index: usize| {
            if let Some(bore) = bores.get(index).cloned() {
                draft.set(bore);
                editing_index.set(Some(index));
            }
        }
    });

    let draft_bore = draft.read().clone();
    rsx! {
        LabeledSelect {
            id: "boreShapeSelector",
            label: "Bore",
            options: select_options_from_enum_iterator(draft_bore.shape(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                if let Some(shape) = OutlineShape::default_from_name(&e.value()) {
                    on_draft_shape_change.call(shape);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            OutlineShapeInput {
                shape: draft_bore.shape().clone(),
                on_shape_change: on_draft_shape_change,
                readonly,
            }
            RowedInputs { inputs: BoreParam::to_input_data_vec(&draft_bore, on_draft_change, readonly) }
        }
        input {
            class: " border-start btn",
            r#type: "button",
            id: "boreSubmit",
            value: if editing_index.read().is_none() { "Add bore" } else { "Update bore" },
            disabled: readonly,
            onclick: {
                let bores = bores;
                move |_| {
                    let bore = draft.read().clone();
                    let index = *editing_index.read();
                    let mut bores = bores.clone();
                    match index.and_then(|index| bores.get_mut(index)) {
                        Some(edited) => *edited = bore,
                        None => bores.push(bore),
                    }
                    editing_index.set(None);
                    on_bores_change.call(bores);
                }
            },
        }
        DynamicListComponent {
            list_entries: bores.iter().map(list_entry).collect::<Vec<_>>(),
            delete_entry_handler,
            modify_entry_handler,
            edit_index: editing_index,
            readonly,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    /// A bore is shifted in meters and turned in degrees, each input leaving the shape and the
    /// other values of the bore as they were; the inputs show the values back in those units.
    #[test]
    fn a_bore_is_placed_by_its_offset_and_rotation() {
        let shape = OutlineShape::from(CircleShape::default());
        let mut bore = Bore::new(shape.clone(), Point2::origin(), degree!(0.0));
        for (param, value) in [
            (BoreParam::X, 0.004),
            (BoreParam::Y, -0.003),
            (BoreParam::Rotation, 30.0),
        ] {
            let mut set = param.setter_from_obj();
            set(&mut bore, value);
        }
        assert_eq!(bore.shape(), &shape);
        assert_eq!(bore.center(), meter!(0.004, -0.003));
        assert_relative_eq!(bore.rotation().get::<degree>(), 30.0, max_relative = 1e-12);
        for (param, shown) in [
            (BoreParam::X, 0.004),
            (BoreParam::Y, -0.003),
            (BoreParam::Rotation, 30.0),
        ] {
            let value = param.create_value_string(&bore).parse::<f64>().unwrap();
            assert_relative_eq!(value, shown, max_relative = 1e-12);
        }
    }
}
