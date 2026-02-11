use crate::components::node_editor::{
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
    node_id: Memo<Uuid>,
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ghost_focus_config_sig = use_signal(|| ghost_focus_config);

    let ghost_focus_config_handler =
        EventHandler::new(move |ghost_focus_config: GhostFocusConfig| {
            on_change.call(NodeChangeEvent {
                node_id: *node_id.read(),
                action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(
                    ghost_focus_config,
                )),
            });
        });

    let max_bounces_handler = EventHandler::new(move |max_bounces: usize| {
        ghost_focus_config_sig.write().set_max_bounces(max_bounces);
        ghost_focus_config_handler.call(*ghost_focus_config_sig.read());
    });
    let fluence_estimator_handler =
        EventHandler::new(move |fluence_estimator: FluenceEstimator| {
            ghost_focus_config_sig
                .write()
                .set_fluence_estimator(fluence_estimator);
            ghost_focus_config_handler.call(*ghost_focus_config_sig.read());
        });

    rsx! {
        FlushableTextInput {
            id: "ghostFocusMaxBounces".to_string(),
            label: "Max Bounces".to_string(),
            value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
            r#type: "number",
            step: "1",
            min: "0",
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            on_save: move |val: String| {
                if let Ok(max_bounces) = val.parse::<usize>() {
                    max_bounces_handler.call(max_bounces);
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
                    fluence_estimator_handler.call(fluence_estimator);
                }
            },
        }
    }
}
