use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::{
        InputData, InputParam,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use approx::relative_eq;
use dioxus::prelude::*;
use inflector::Inflector;
use nalgebra::Vector3;
use opossum_core::utils::{
    default_from_name::DefaultFromName, geom_transformation::TranslationAxis,
};
use std::fmt::Display;
use strum::EnumIter;
use uuid::Uuid;

#[derive(PartialEq, Eq, EnumIter, Clone, Copy)]
enum Vec3Options {
    X,
    Y,
    Z,
    Mix,
}

impl Display for Vec3Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => write!(f, "X"),
            Self::Y => write!(f, "Y"),
            Self::Z => write!(f, "Z"),
            Self::Mix => write!(f, "Mix"),
        }
    }
}

impl DefaultFromName for Vec3Options {}

#[component]
pub fn Vec3Editor(
    node_id: ReadSignal<Uuid>,
    vector: Vector3<f64>,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let select_label = property_key.to_sentence_case();
    let vec_sig = use_synced_signal(vector);

    // Create the save handler to propagate changes up to the core
    let on_save = on_save_proptype_handler(vec_sig, property_key.clone(), on_change, node_id);

    let dummy_legacy_callback = EventHandler::new(|_| {});
    let vec_x_input = InputData::new(
        InputParam::F64(format!("{select_label} x")),
        format!("vec3xProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        on_vec_input_change_str(vec_sig.into(), TranslationAxis::X, on_save),
        format!("{}", vec_sig.read().x),
        readonly,
    );

    let dummy_legacy_callback = EventHandler::new(|_| {});
    let vec_y_input = InputData::new(
        InputParam::F64(format!("{select_label} y")),
        format!("vec3yProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        on_vec_input_change_str(vec_sig.into(), TranslationAxis::Y, on_save),
        format!("{}", vec_sig.read().y),
        readonly,
    );

    let dummy_legacy_callback = EventHandler::new(|_| {});
    let vec_z_input = InputData::new(
        InputParam::F64(format!("{select_label} z")),
        format!("vec3zProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        on_vec_input_change_str(vec_sig.into(), TranslationAxis::Z, on_save),
        format!("{}", vec_sig.read().z),
        readonly,
    );

    // Determine which dropdown option matches the current vector state
    let vec3_select = use_memo(move || {
        let current_vec = vec_sig.read();

        // Safe check for zero vector to avoid NaN results from normalization
        if relative_eq!(current_vec.norm(), 0.0) {
            Vec3Options::Mix
        } else {
            let normed_vec = current_vec.normalize();
            if relative_eq!(normed_vec.y, 0.0) && relative_eq!(normed_vec.z, 0.0) {
                Vec3Options::X
            } else if relative_eq!(normed_vec.x, 0.0) && relative_eq!(normed_vec.z, 0.0) {
                Vec3Options::Y
            } else if relative_eq!(normed_vec.x, 0.0) && relative_eq!(normed_vec.y, 0.0) {
                Vec3Options::Z
            } else {
                Vec3Options::Mix
            }
        }
    });

    rsx! {
        LabeledSelect {
            id: format!("vec3Property{property_key}").to_camel_case(),
            label: select_label,
            options: select_options_from_enum_iterator(&vec3_select(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                if let Some(vec_3_opt) = Vec3Options::default_from_name(&e.data.value()) {
                    let new_vec = match vec_3_opt {
                        Vec3Options::X => Vector3::new(1., 0., 0.),
                        Vec3Options::Y => Vector3::new(0., 1., 0.),
                        Vec3Options::Z => Vector3::new(0., 0., 1.),
                        Vec3Options::Mix => Vector3::new(1., 1., 1.),
                    };
                    on_save.call(new_vec);
                }
            },
        }
        {
            if vec3_select() == Vec3Options::Mix {
                rsx! {
                    RowedInputs { inputs: vec![vec_x_input, vec_y_input, vec_z_input] }
                }
            } else {
                rsx! {}
            }
        }
    }
}

fn on_vec_input_change_str(
    vec_sig: ReadSignal<Vector3<f64>>,
    axis: TranslationAxis,
    on_vec_change: EventHandler<Vector3<f64>>,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(val) = val_str.parse::<f64>() {
            let mut vec = *vec_sig.read();
            match axis {
                TranslationAxis::X => vec.x = val,
                TranslationAxis::Y => vec.y = val,
                TranslationAxis::Z => vec.z = val,
            }
            on_vec_change.call(vec);
        }
    })
}
