use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::hooks::use_save_manager;
use crate::components::node_editor::inputs::input_components::FormContext;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{GraphStore, GraphStoreAction, NodeType};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_core::nodes::fluence_detector::Fluence;
use opossum_core::prelude::{AnalyzerType, Isometry, Properties, Proptype};
use std::ops::AddAssign;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct NodeChangeEvent {
    pub node_id: Uuid,
    pub action: NodeChangeAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(String),
    Lidt(Fluence),
    Alignment(Isometry),
    Inverted(bool),
    Property(String, Proptype),
    Isometry(Option<Isometry>),
    AnalyzerType(AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(
    active_node_opt: Memo<Option<(NodeType, Uuid)>>,
    is_modified: Signal<bool>,
) -> Element {
    let node_properties_sig = use_signal(Properties::default);
    use_context_provider(|| node_properties_sig);

    // 1. SETUP PROTOCOLS (mut Signals) via shared hook
    let save_manager = use_save_manager();
    let mut flush_trigger = save_manager.flush_trigger;
    let dirty_count = save_manager.dirty_count;

    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    // 2. STATE
    #[allow(clippy::redundant_closure)]
    let mut displayed_node = use_signal(|| active_node_opt());

    // 3. FLUSH COMMAND
    use_effect(move || {
        active_node_opt(); // subscribe to node_id changes
        flush_trigger.write().add_assign(1);
    });

    // 4. THE GATEKEEPER
    use_effect(move || {
        let target = active_node_opt();
        let current = displayed_node();
        let pending = *dirty_count.read();

        // If target differs from current...
        if target != current {
            // ... and everything is clean...
            if pending == 0 {
                // ... then switch
                displayed_node.set(target);
            }
        }
    });

    // Standard Processing
    use_node_config_processor(node_properties_sig, is_modified);
    let node_config_processor = use_coroutine_handle::<NodeChangeEvent>();
    let on_node_change = EventHandler::new(move |evt: NodeChangeEvent| {
        node_config_processor.send(evt);
    });

    match displayed_node() {
        Some((NodeType::Optical(_), node_id)) => rsx! {
            OpticalNodeEditor {
                node_id,
                node_properties_sig,
                on_change: on_node_change,
            }
        },
        Some((NodeType::Analyzer(_), node_id)) => rsx! {
            AnalyzerNodeEditor { node_id, on_change: on_node_change }
        },
        None => rsx! {
            div { "No node selected" }
        },
    }
}

fn use_node_config_processor(
    mut node_properties_sig: Signal<Properties>,
    mut is_modified: Signal<bool>,
) {
    let graph_processor = use_coroutine_handle::<GraphStoreAction>();
    let mut graph_store = use_context::<Signal<GraphStore>>();

    use_coroutine(
        move |mut rx: UnboundedReceiver<NodeChangeEvent>| async move {
            while let Some(event) = rx.next().await {
                let uuid = event.node_id;

                let result: Result<(), String> = match event.action {
                    NodeChangeAction::Name(name) => {
                        api::update_node_name(uuid, name.clone()).await.map(|_| {
                            graph_store.write().set_name_of_node(uuid, name);
                        })
                    }
                    NodeChangeAction::Lidt(lidt_new) => {
                        api::update_node_lidt(uuid, lidt_new).await.map(|_| ())
                    }
                    NodeChangeAction::Alignment(iso) => {
                        api::update_node_alignment(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Property(key, prop) => {
                        api::update_node_property(uuid, (key.clone(), prop.clone()))
                            .await
                            .map(|_| {
                                if let Some(active_id) = graph_store.read().active_node()
                                    && active_id == uuid
                                {
                                    node_properties_sig.write().set(&key, prop).unwrap_or_else(
                                        |_| {
                                            OPOSSUM_UI_LOGS.write().add_log(&format!(
                                                "Failed to set local property: {key}"
                                            ));
                                        },
                                    );
                                }
                            })
                    }
                    NodeChangeAction::Isometry(iso) => {
                        api::update_node_isometry(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Inverted(inverted) => {
                        match api::update_node_inversion(uuid, inverted).await {
                            Ok(connections) => {
                                graph_processor.send(GraphStoreAction::UpdateEdges(connections));
                                graph_store.write().set_node_inverted(uuid, inverted);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    }
                    NodeChangeAction::AnalyzerType(analyzer_type) => {
                        api::update_analyzer_config_ron(uuid, analyzer_type)
                            .await
                            .map(|_| ())
                    }
                };

                match result {
                    Ok(()) => {
                        is_modified.set(true);
                    }
                    Err(err_str) => {
                        OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    }
                }
            }
        },
    );
}
