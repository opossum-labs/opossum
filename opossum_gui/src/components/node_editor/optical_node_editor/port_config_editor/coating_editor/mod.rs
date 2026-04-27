use dioxus::prelude::*;
use opossum_core::{
    coatings::{CoatingConstantR, CoatingType},
    utils::default_from_name::DefaultFromName,
};

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::ElementList,
        inputs::{
            input_components::{LabeledSelect, NodeConfigPlainF64Input},
            select_options_from_enum_iterator,
        },
    },
};

#[component]
pub fn CoatingEditor(
    coating_type: ReadSignal<CoatingType>,
    on_change: EventHandler<CoatingType>,
    readonly: bool,
) -> Element {
    let on_coating_changed = move |ct: CoatingType| on_change.call(ct);
    let mut element_list: Vec<Result<VNode, RenderError>> = vec![rsx! {
        CoatingTypeSelector {coating_type, on_change: on_coating_changed }
    }];
    if let CoatingType::ConstantR(conf) = &*coating_type.read() {
        element_list.push(rsx! {
            NodeConfigPlainF64Input {
                id: "coatingReflectivityInput",
                label: "Reflectivity",
                value: conf.reflectivity(),
                onchange: move |val: f64| {
                    if let Ok(new_conf) = CoatingConstantR::new(val) {
                        on_change.call(new_conf.into());
                    } else {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log("Invalid reflectivity value");
                    }
                },
                readonly,
            }
        });
    }
    rsx! {
        div { class: "form-floating border-start",
            p { class: "form-label text-secondary", "Coating Type" }
            ElementList { element_list }
        }
    }
}

#[component]
pub fn CoatingTypeSelector(
    coating_type: ReadSignal<CoatingType>,
    on_change: EventHandler<CoatingType>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "nodeFilterTypeSelector",
            label: "Coating type definition",
            options: select_options_from_enum_iterator(&*coating_type.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ftb) = CoatingType::default_from_name(val.as_str()) {
                    on_change.call(ftb);
                }
            },
        }
    }
}
