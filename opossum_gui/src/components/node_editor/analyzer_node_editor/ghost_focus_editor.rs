use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::{
    prelude::{AnalyzerType, GhostFocusConfig},
    surface::hit_map::fluence_estimator::FluenceEstimator,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn GhostFocusEditor(
    node_id: Uuid,
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ghost_focus_config_sig = use_signal(|| ghost_focus_config.clone());
    use_update_signal_with_reactive_prop(ghost_focus_config, ghost_focus_config_sig);

    rsx! {
        FlushableTextInput {
            id: "ghostFocusMaxBounces".to_string(),
            label: "Max Bounces".to_string(),
            value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
            r#type: "number",
            step: "1",
            min: "0",
            on_save: move |val: String| {
                if let Ok(max_bounces) = val.parse::<usize>() {
                    ghost_focus_config_sig.write().set_max_bounces(max_bounces);
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::GhostFocus(ghost_focus_config_sig.read().clone()),
                            ),
                        });
                }
            },
        }
        LabeledSelect {
            id: "ghostFocusFluence".to_string(),
            label: "Fluence Estimator".to_string(),
            options: select_options_from_enum_iterator(
                ghost_focus_config_sig.read().fluence_estimator(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(fluence_estimator) = FluenceEstimator::default_from_name(
                    val.as_str(),
                ) {
                    ghost_focus_config_sig.write().set_fluence_estimator(fluence_estimator);
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::GhostFocus(ghost_focus_config_sig.read().clone()),
                            ),
                        });
                }
            },
        }
    }
}
