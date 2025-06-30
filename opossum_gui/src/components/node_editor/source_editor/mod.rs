#![allow(clippy::derive_partial_eq_without_eq)]
pub mod energy_distribution;
pub mod light_data_builder_selection;
pub mod position_distribution;
pub mod ray_type_selection;
pub mod spectral_distribution;

use std::{cell::RefCell, fmt::Display, rc::Rc};

pub use energy_distribution::*;
use itertools::Itertools;
pub use light_data_builder_selection::*;
use opossum_backend::{light_data_builder::LightDataBuilder, Proptype};
pub use position_distribution::*;
pub use ray_type_selection::*;
pub use spectral_distribution::*;

use crate::components::node_editor::accordion::{AccordionItem, LabeledInput};

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    light_data_builder_opt: Option<LightDataBuilder>,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let mut light_data_builder_sig = Signal::new(LightDataBuilderHistory::default());

    use_effect(move || {
        prop_type_sig.set(Proptype::LightDataBuilder(
            light_data_builder_sig.read().get_current().cloned(),
        ));
    });

    use_effect(move || {
        let (ld_builder, key) = match &light_data_builder_opt {
            Some(ld) if matches!(ld, LightDataBuilder::Geometric(_)) => (ld.clone(), "Rays"),
            Some(ld) => (ld.clone(), "Energy"),
            _ => (LightDataBuilder::default(), "Rays"),
        };
        light_data_builder_sig
            .with_mut(|ldb| ldb.replace_or_insert_and_set_current(key, ld_builder));
    });

    let accordion_item_content = rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig }
        RayDataBuilderSelector { light_data_builder_sig }
        ReferenceLengthEditor { light_data_builder_sig }
        DistributionEditor { light_data_builder_sig }
        ImageSourceEditor { light_data_builder_sig }
    };
    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark ",
            id: "accordionLightDataConfig",
            AccordionItem {
                elements: vec![accordion_item_content],
                header: "Light Definition",
                header_id: "sourceHeading",
                parent_id: "accordionLightDataConfig",
                content_id: "sourceCollapse",
            }
        }
    }
}

#[component]
pub fn DistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (is_rays, is_not_image) = light_data_builder_sig.read().is_rays_is_not_image();

    if is_rays && is_not_image {
        rsx! {
            div {
                class: "accordion accordion-borderless bg-dark border-start",
                id: "accordionSourceDists",
                PositionDistributionEditor { light_data_builder_sig }
                EnergyDistributionEditor { light_data_builder_sig }
                SpectralDistributionEditor { light_data_builder_sig }
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn DistLabeledInput(dist_input: DistInput) -> Element {
    if dist_input.dist_param == InputParam::Rectangular {
        let label = dist_input.dist_param.input_label();
        rsx! {
            div {
                class: "form-floating-checkbox border-start",
                "data-mdb-input-init": "",
                label { class: "text-secondary", r#for: dist_input.id.clone(), "{label}" }
                br {}
                input {
                    class: "form-check-input text-light",
                    id: dist_input.id.as_str(),
                    name: dist_input.id.as_str(),
                    value: dist_input.value.clone(),
                    r#type: "checkbox",
                    role: "switch",
                    checked: dist_input.value.parse::<bool>().unwrap_or_default(),
                    onchange: move |e| dist_input.callback_opt.call(e),
                }
            }
        }
    } else if dist_input.dist_param == InputParam::FilePath {
        let label = format!(
            "{}: {}",
            dist_input.dist_param.input_label(),
            dist_input.value
        );
        rsx! {
            div {
                id: "imgSrcFileSelection",
                class: "form-file border-start",
                "data-mdb-input-init": "",
                input {
                    class: "form-input text-light",
                    id: dist_input.id.as_str(),
                    r#type: "file",
                    onchange: move |e| dist_input.callback_opt.call(e),
                }
                label {
                    id: "imgSrcFileSelectionLabel",
                    class: "btn bg-dark text-secondary",
                    r#for: dist_input.id,
                    "{label}"
                }
            }
        }
    } else {
        rsx! {
            LabeledInput {
                id: dist_input.id,
                label: dist_input.dist_param.input_label(),
                value: dist_input.value,
                step: dist_input.dist_param.step_value(),
                min: dist_input.dist_param.min_value(),
                onchange: dist_input.callback_opt,
                r#type: "number",
            }
        }
    }
}

#[component]
pub fn RowedInputs(inputs: Vec<DistInput>) -> Element {
    rsx! {
        for chunk in inputs.iter().chunks(2) {
            {
                let inputs: Vec<&DistInput> = chunk.collect::<Vec<&DistInput>>();
                if inputs.len() == 2 {
                    rsx! {
                        div { class: "row gy-1 gx-2",
                            div { class: "col-sm",
                                DistLabeledInput { dist_input: inputs[0].clone() }
                            }
                            div { class: "col-sm",
                                DistLabeledInput { dist_input: inputs[1].clone() }
                            }
                        }
                    }
                } else if inputs.len() == 1 {
                    rsx! {
                        DistLabeledInput { dist_input: inputs[0].clone() }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

pub struct CallbackWrapper(Rc<RefCell<dyn FnMut(Event<FormData>) + 'static>>);

impl PartialEq for CallbackWrapper {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl CallbackWrapper {
    pub fn new<F>(f: F) -> Self
    where
        F: FnMut(Event<FormData>) + 'static,
    {
        Self(Rc::new(RefCell::new(f)))
    }

    pub fn call(&self, e: Event<FormData>) {
        (self.0.borrow_mut())(e);
    }
    #[must_use]
    pub fn noop() -> Self {
        Self::new(|_| {})
    }
}

impl Clone for CallbackWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone, PartialEq)]
pub struct DistInput {
    pub value: String,
    pub id: String,
    pub dist_param: InputParam,
    pub callback_opt: CallbackWrapper,
}

impl DistInput {
    pub fn new(
        dist_param: InputParam,
        dist_type: &impl Display,
        callback_opt: CallbackWrapper,
        value: String,
    ) -> Self {
        Self {
            value,
            id: format!("node{dist_type}{dist_param}Input"),
            dist_param,
            callback_opt,
        }
    }
}
