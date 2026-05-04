use dioxus::prelude::*;
use opossum_core::{
    coatings::{CoatingConstantR, CoatingType},
    percent,
    utils::default_from_name::DefaultFromName,
};
use uom::si::ratio::percent;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::ElementList,
        inputs::{
            input_components::{LabeledSelect, NodeConfigUnitInput, UnitHandling},
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
    let mut coating_type_sig = use_signal(|| *coating_type.read());
    let on_coating_changed = move |ct: CoatingType| {
        coating_type_sig.set(ct);
        on_change.call(ct)
    };
    let mut element_list: Vec<Result<VNode, RenderError>> = vec![rsx! {
        CoatingTypeSelector {coating_type_sig, on_change: on_coating_changed }
    }];
    if let CoatingType::ConstantR(conf) = &*coating_type_sig.read() {
        element_list.push(rsx! {
            NodeConfigUnitInput {
                id: "coatingReflectivityInput",
                label: "Reflectivity",
                value: conf.reflectivity().get::<percent>(),
                unit_config: UnitHandling::new("%", false),
                onchange: move |val: f64| {
                    if let Ok(new_conf) = CoatingConstantR::new(percent!(val)) {
                        on_change.call(new_conf.into());
                    } else {
                        OPOSSUM_UI_LOGS.write().add_log("Invalid reflectivity value");
                    }
                },
                readonly,
            }
        });
    }
    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn CoatingTypeSelector(
    coating_type_sig: ReadSignal<CoatingType>,
    on_change: EventHandler<CoatingType>,
) -> Element {
    rsx! {
        LabeledSelect {
            id: "nodeFilterTypeSelector",
            label: "Coating type definition",
            options: select_options_from_enum_iterator(&*coating_type_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ftb) = CoatingType::default_from_name(val.as_str()) {
                    on_change.call(ftb);
                }
            },
        }
    }
}
