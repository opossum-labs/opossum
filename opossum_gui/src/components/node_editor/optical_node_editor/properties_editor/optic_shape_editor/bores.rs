use super::OutlineShapeInput;
use crate::components::node_editor::{
    accordion::AccordionItem,
    hooks::use_synced_signal,
    inputs::{
        InputParam, IntoInputData, IntoInputDataStrings,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    apertures::{Bore, Outline, OutlineShape},
    degree, meter,
    utils::default_from_name::DefaultFromName,
};
use strum::EnumIter;
use uom::si::angle::degree;

/// Where a bore lies: its offset from the axis, its rotation about its center and its tilt
/// against the axis.
#[derive(Clone, Copy, PartialEq, Debug, Eq, EnumIter)]
enum BoreParam {
    X,
    Y,
    Rotation,
    TiltX,
    TiltY,
}

impl From<BoreParam> for InputParam {
    fn from(value: BoreParam) -> Self {
        match value {
            BoreParam::X => Self::SIUnit("Offset x".into(), "m".into()),
            BoreParam::Y => Self::SIUnit("Offset y".into(), "m".into()),
            BoreParam::Rotation => Self::SIUnit("Rotation".into(), "°".into()),
            BoreParam::TiltX => Self::SIUnit("Tilt x".into(), "°".into()),
            BoreParam::TiltY => Self::SIUnit("Tilt y".into(), "°".into()),
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
            Self::TiltX => format!("{}", bore.tilt().0.get::<degree>()),
            Self::TiltY => format!("{}", bore.tilt().1.get::<degree>()),
        }
    }
}

impl IntoInputData<f64, Bore, Bore> for BoreParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut Bore, f64) {
        let param = *self;
        move |bore: &mut Bore, value: f64| {
            let (mut center, mut rotation, mut tilt) =
                (bore.center(), bore.rotation(), bore.tilt());
            match param {
                Self::X => center.x = meter!(value),
                Self::Y => center.y = meter!(value),
                Self::Rotation => rotation = degree!(value),
                Self::TiltX => tilt.0 = degree!(value),
                Self::TiltY => tilt.1 = degree!(value),
            }
            *bore = Bore::new(bore.shape().clone(), center, rotation).with_tilt(tilt.0, tilt.1);
        }
    }
}

/// Editor for the bore through a component: a collapsed section with the bore being drafted
/// (shape, offset and rotation), starting as the bore the optic shape has. Whether a component
/// has a bore is up to its kind, so the bore can be changed but neither added nor removed.
///
/// Updating the bore hands it on once, so it is one step to undo. Editing the draft hands
/// nothing on; while the core refuses the outline the draft would make ([`Outline::new`]), the
/// button looks refused, does nothing and tells why on hover.
///
/// # Arguments
///
/// * `edge` - the edge of the component shown, which the bore has to lie within.
/// * `bore` - the bore the optic shape has.
/// * `on_bore_change` - handler receiving the changed bore.
/// * `readonly` - whether the inputs are shown read-only.
#[component]
pub fn BoreEditor(
    edge: OutlineShape,
    bore: Bore,
    on_bore_change: EventHandler<Bore>,
    readonly: bool,
) -> Element {
    let mut draft = use_synced_signal(bore);

    let on_draft_change = EventHandler::new(move |bore: Bore| draft.set(bore));
    let on_draft_shape_change = EventHandler::new(move |shape: OutlineShape| {
        let (center, rotation, tilt) = {
            let bore = draft.read();
            (bore.center(), bore.rotation(), bore.tilt())
        };
        draft.set(Bore::new(shape, center, rotation).with_tilt(tilt.0, tilt.1));
    });

    let draft_bore = draft.read().clone();
    let submitted = draft_bore.clone();
    let refusal = (!readonly)
        .then(|| Outline::new(edge, Some(draft_bore.clone())).err())
        .flatten()
        .map(|refusal| refusal.to_string());
    let refused = refusal.is_some();
    let draft_inputs = rsx! {
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
            class: if refused { "border-start btn refused" } else { "border-start btn" },
            r#type: "button",
            id: "boreSubmit",
            value: "Update bore",
            disabled: readonly,
            // Refused, the button stays hoverable so that its title can say why.
            "aria-disabled": refused,
            title: refusal.unwrap_or_default(),
            onclick: move |_| {
                if !refused {
                    on_bore_change.call(submitted.clone());
                }
            },
        }
    };
    rsx! {
        AccordionItem {
            elements: vec![draft_inputs],
            header: "Bore",
            header_id: "boreEditorHeading",
            parent_id: "boreEditorAccordion",
            content_id: "boreEditorCollapse",
            level: 2,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;
    use nalgebra::Point2;
    use opossum_core::apertures::CircleShape;

    /// A bore is shifted in meters, turned and tilted in degrees, each input leaving the shape and
    /// the other values of the bore as they were; the inputs show the values back in those units.
    #[test]
    fn a_bore_is_placed_by_its_offset_rotation_and_tilt() {
        let shape = OutlineShape::from(CircleShape::default());
        let mut bore = Bore::new(shape.clone(), Point2::origin(), degree!(0.0));
        let values = [
            (BoreParam::X, 0.004),
            (BoreParam::Y, -0.003),
            (BoreParam::Rotation, 30.0),
            (BoreParam::TiltX, -10.0),
            (BoreParam::TiltY, 45.0),
        ];
        for (param, value) in values {
            let mut set = param.setter_from_obj();
            set(&mut bore, value);
        }
        assert_eq!(bore.shape(), &shape);
        assert_eq!(bore.center(), meter!(0.004, -0.003));
        assert_relative_eq!(bore.rotation().get::<degree>(), 30.0, max_relative = 1e-12);
        assert_relative_eq!(bore.tilt().0.get::<degree>(), -10.0, max_relative = 1e-12);
        assert_relative_eq!(bore.tilt().1.get::<degree>(), 45.0, max_relative = 1e-12);
        for (param, shown) in values {
            let value = param.create_value_string(&bore).parse::<f64>().unwrap();
            assert_relative_eq!(value, shown, max_relative = 1e-12);
        }
    }
}
