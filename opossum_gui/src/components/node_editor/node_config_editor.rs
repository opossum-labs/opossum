use crate::components::node_editor::analyzer_node_editor::AnalyzerNodeEditor;
use crate::components::node_editor::optical_node_editor::OpticalNodeEditor;
use crate::components::scenery_editor::{GraphStore, GraphStoreAction, NodeType};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use futures_util::StreamExt;
use opossum_core::nodes::fluence_detector::Fluence;
use opossum_core::prelude::{AnalyzerType, Isometry, Properties, Proptype};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeChangeAction {
    Name(Uuid, String),
    Lidt(Uuid, Fluence),
    Alignment(Uuid, Isometry),
    Inverted(Uuid, bool),
    Property(Uuid, String, Proptype),
    Isometry(Uuid, Option<Isometry>),
    AnalyzerType(Uuid, AnalyzerType),
}

#[component]
pub fn NodeConfigEditor(
    active_node_opt: Memo<Option<(NodeType, Uuid)>>,
    is_modified: Signal<bool>,
) -> Element {
    let node_properties_sig = use_signal(Properties::default);

    // Props reichen hier eigentlich, Context ist optional, aber wir lassen es wie gehabt
    use_context_provider(|| node_properties_sig);
    use_node_config_processor(node_properties_sig, is_modified);
    match active_node_opt() {
        Some((NodeType::Optical(_), node_id)) => rsx! {
            OpticalNodeEditor { node_id, node_properties_sig }
        },
        Some((NodeType::Analyzer(_), node_id)) => rsx! {
            AnalyzerNodeEditor { node_id }
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
        move |mut rx: UnboundedReceiver<NodeChangeAction>| async move {
            while let Some(action) = rx.next().await {
                let result: Result<(), String> = match action {
                    NodeChangeAction::Name(uuid, name) => {
                        api::update_node_name(uuid, name.clone()).await.map(|_| {
                            // Store nur updaten, wenn der Node noch existiert
                            graph_store.write().set_name_of_node(uuid, name);
                        })
                    }
                    NodeChangeAction::Lidt(uuid, lidt_new) => {
                        api::update_node_lidt(uuid, lidt_new).await.map(|_| ())
                    }
                    NodeChangeAction::Alignment(uuid, iso) => {
                        api::update_node_alignment(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Property(uuid, key, prop) => {
                        api::update_node_property(uuid, (key.clone(), prop.clone()))
                            .await
                            .map(|_| {
                                // Wir müssen prüfen, ob der bearbeitete Node *immer noch* der aktive ist,
                                // bevor wir das lokale UI Signal updaten. Sonst zeigen wir falsche Daten an.
                                if let Some(active_id) = graph_store.read().active_node() {
                                    if active_id == uuid {
                                        node_properties_sig.write().set(&key, prop).unwrap_or_else(
                                            |_| {
                                                OPOSSUM_UI_LOGS.write().add_log(&format!(
                                                    "Failed to set local property: {key}"
                                                ));
                                            },
                                        );
                                    }
                                }
                            })
                    }
                    NodeChangeAction::Isometry(uuid, iso) => {
                        api::update_node_isometry(uuid, iso).await.map(|_| ())
                    }
                    NodeChangeAction::Inverted(uuid, inverted) => {
                        match api::update_node_inversion(uuid, inverted).await {
                            Ok(connections) => {
                                graph_processor.send(GraphStoreAction::UpdateEdges(connections));
                                graph_store.write().set_node_inverted(uuid, inverted);
                                Ok(())
                            }
                            Err(e) => Err(e),
                        }
                    }
                    NodeChangeAction::AnalyzerType(uuid, analyzer_type) => {
                        api::update_analyzer_config_ron(uuid, analyzer_type)
                            .await
                            .map(|_| ())
                    }
                };

                match result {
                    Ok(_) => {
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
