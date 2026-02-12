use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        inputs::{
            InputData, InputParam,
            input_components::{LabeledSelect, RowedInputs},
            select_options_from_enum_iterator,
        },
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
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
    node_id: Memo<Uuid>,
    vector: Vector2<f64>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let select_label = property_key.to_sentence_case();
    let vec_sig = use_signal(|| vector);

    let on_save =
        on_save_proptype_handler(vec_sig, property_key.clone(), on_change, node_id.into());

    let dummy_legacy_callback = EventHandler::new(|_| {});

    let vec_x_input = InputData::new(
        InputParam::F64(format!("{select_label} x")),
        format!("vec2xProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        on_vec_input_change_str(vec_sig.into(), TranslationAxis::X, on_save),
        format!("{:.3}", vec_sig.read().x),
    );

    let dummy_legacy_callback = EventHandler::new(|_| {});

    let vec_y_input = InputData::new(
        InputParam::F64(format!("{select_label} y")),
        format!("vec2yProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        on_vec_input_change_str(vec_sig.into(), TranslationAxis::Y, on_save),
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
                    let new_vec = match vec_2_opt {
                        Vec2Options::X => Vector2::new(1., 0.),
                        Vec2Options::Y => Vector2::new(0., 1.),
                        Vec2Options::Mix => Vector2::new(1., 1.),
                    };
                    on_save.call(new_vec);
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

fn on_vec_input_change_str(
    vec_sig: ReadSignal<Vector2<f64>>,
    axis: TranslationAxis,
    on_vec_change: EventHandler<Vector2<f64>>,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(val) = val_str.parse::<f64>() {
            let mut vec = *vec_sig.read();
            match axis {
                TranslationAxis::X => vec.x = val,
                TranslationAxis::Y => vec.y = val,
                TranslationAxis::Z => OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Z-axis is not valid vor Vec2 Proptype!"),
            }
            on_vec_change.call(vec);
        }
    })
}
