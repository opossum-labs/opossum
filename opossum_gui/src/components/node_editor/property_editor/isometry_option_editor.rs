use crate::{
    components::node_editor::{
        accordion::AccordionItem, inputs::input_components::LabeledInput, CallbackWrapper,
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{degree, millimeter, Proptype, RotationAxis, TranslationAxis};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn IsometryOptionEditor(property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    if let Proptype::Isometry(iso_opt) = &*prop_type_sig.read() {
        let iso = iso_opt.unwrap_or_default();
        let mut editor_inputs = Vec::<Result<VNode, RenderError>>::new();
        for axis in TranslationAxis::iter() {
            editor_inputs.push(rsx!{
                LabeledInput {
                    id: format!("isometryOption{axis}Property{property_key}").to_camel_case(),
                    label: format!("{} translation in mm", axis),
                    value: format!("{:.3}", iso.translation_of_axis(axis).get::<millimeter>()),
                    r#type: "number",
                    onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                        let mut iso = iso;
                        if let Ok(trans) = e.data.value().parse::<f64>() {
                            match iso.set_translation_of_axis(axis, millimeter!(trans)) {
                                Ok(()) => {
                                    prop_type_sig.set(Proptype::Isometry(Some(iso)));
                                }
                                Err(err_str) => {
                                    OPOSSUM_UI_LOGS
                                        .write()
                                        .add_log(
                                            format!(
                                                "Failed to set translation for axis {axis}: {err_str}",
                                            )
                                                .as_str(),
                                        );
                                }
                            }
                        }
                    }),
                }
            });
        }
        for axis in RotationAxis::iter() {
            editor_inputs.push(rsx!{
                LabeledInput {
                    id: format!("isometryOption{axis}Property{property_key}").to_camel_case(),
                    label: format!("{} rotation in degree", axis),
                    value: format!("{:.3}", iso.rotation_of_axis(axis).get::<degree>()),
                    r#type: "number",
                    onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                        let mut iso = iso;
                        if let Ok(rot) = e.data.value().parse::<f64>() {
                            match iso.set_rotation_of_axis(axis, degree!(rot)) {
                                Ok(()) => {
                                    prop_type_sig.set(Proptype::Isometry(Some(iso)));
                                }
                                Err(err_str) => {
                                    OPOSSUM_UI_LOGS
                                        .write()
                                        .add_log(
                                            format!("Failed to set rotation for axis {axis}: {err_str}")
                                                .as_str(),
                                        );
                                }
                            }
                        }
                    }),
                }
            });
        }

        rsx! {
            div {
                class: "accordion accordion-borderless bg-dark border-start",
                id: "accordionIsometryOptionConfig",
                AccordionItem {
                    elements: editor_inputs,
                    header: "Source isometry",
                    header_id: "srcIsometryHeading",
                    parent_id: "accordionIsometryOptionConfig",
                    content_id: "srcIsometryCollapse",
                }
            }
        }
    } else {
        rsx! {}
    }
}
