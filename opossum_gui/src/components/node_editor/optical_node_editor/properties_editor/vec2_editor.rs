use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        CallbackWrapper,
        inputs::{
            InputData, InputParam,
            input_components::{LabeledSelect, RowedInputs},
            select_options_from_enum_iterator,
        },
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::{
            use_set_node_change_property, use_update_signal_with_reactive_prop,
        },
    },
};
use approx::relative_eq;
use dioxus::prelude::*;
use inflector::Inflector;
use nalgebra::Vector2;
use opossum_core::utils::{
    default_from_name::DefaultFromName, geom_transformation::TranslationAxis,
};
use std::fmt::Display;
use strum::EnumIter;
use uuid::Uuid;

#[derive(PartialEq, Eq, EnumIter, Clone, Copy)]
enum Vec2Options {
    X,
    Y,
    Mix,
}

impl Display for Vec2Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => write!(f, "X"),
            Self::Y => write!(f, "Y"),
            Self::Mix => write!(f, "Mix"),
        }
    }
}

impl DefaultFromName for Vec2Options {}

#[component]
pub fn Vec2Editor(
    node_id: Uuid,
    vector: Vector2<f64>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let select_label = property_key.to_sentence_case();
    let mut vec_sig = use_signal(|| vector);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        vector,
        vec_sig,
        on_change,
    );

    let vec_x_input = InputData::new(
        InputParam::F64(format!("{select_label} x")),
        format!("vec2xProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_vec_input_change(vec_sig, TranslationAxis::X),
        format!("{:.3}", vec_sig.read().x),
    );
    let vec_y_input = InputData::new(
        InputParam::F64(format!("{select_label} y")),
        format!("vec2yProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_vec_input_change(vec_sig, TranslationAxis::Y),
        format!("{:.3}", vec_sig.read().y),
    );
    let vec2_select = use_memo(move || {
        let current_vec = vec_sig.read();
        let normed_vec = current_vec.normalize();
        if relative_eq!(normed_vec.x, 0.0) {
            Vec2Options::Y
        } else if relative_eq!(normed_vec.y, 0.0) {
            Vec2Options::X
        } else {
            Vec2Options::Mix
        }
    });

    rsx! {
        LabeledSelect {
            id: format!("vec2Property{property_key}").to_camel_case(),
            label: select_label,
            options: select_options_from_enum_iterator(&vec2_select(), None),
            onchange: move |e: Event<FormData>| {
                if let Some(vec_2_opt) = Vec2Options::default_from_name(&e.data.value()) {
                    match vec_2_opt {
                        Vec2Options::X => vec_sig.set(Vector2::new(1., 0.)),
                        Vec2Options::Y => vec_sig.set(Vector2::new(0., 1.)),
                        Vec2Options::Mix => vec_sig.set(Vector2::new(1., 1.)),
                    }
                }
            },
        }
        {
            if vec2_select() == Vec2Options::Mix {
                rsx! {
                    RowedInputs { inputs: vec![vec_x_input, vec_y_input] }
                }
            } else {
                rsx! {}
            }
        }
    }
}

fn on_vec_input_change(
    mut vec_sig: Signal<Vector2<f64>>,
    axis: TranslationAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            match axis {
                TranslationAxis::X => vec_sig.write().x = val,
                TranslationAxis::Y => vec_sig.write().y = val,
                TranslationAxis::Z => OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Z-axis is not valid vor Vec2 Proptype!"),
            }
        }
    })
}
