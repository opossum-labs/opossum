use crate::OPOSSUM_UI_LOGS;
use crate::components::node_editor::inputs::input_components::NodeConfigUnitInput;
use crate::components::node_editor::{
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::{
    analyzers::raytrace::MissedSurfaceStrategy, joule, prelude::*,
    utils::default_from_name::DefaultFromName,
};
use uom::si::f64::Energy;
use uuid::Uuid;

#[component]
pub fn RayTraceEditor(
    node_id: Memo<Uuid>,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ray_trace_config_sig = use_signal(|| ray_trace_config);

    let ray_trace_config_handler = EventHandler::new(move |ray_trace_config: RayTraceConfig| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(ray_trace_config)),
        });
    });

    let max_refractions_handler = EventHandler::new(move |max_refractions: usize| {
        ray_trace_config_sig
            .write()
            .set_max_number_of_refractions(max_refractions);
        ray_trace_config_handler.call(*ray_trace_config_sig.read());
    });
    let max_bounces_handler = EventHandler::new(move |max_bounces: usize| {
        ray_trace_config_sig
            .write()
            .set_max_number_of_bounces(max_bounces);
        ray_trace_config_handler.call(*ray_trace_config_sig.read());
    });
    let min_ray_energy_handler = EventHandler::new(move |min_ray_energy: Energy| {
        if ray_trace_config_sig
            .write()
            .set_min_energy_per_ray(min_ray_energy).is_ok()
        {
            ray_trace_config_handler.call(*ray_trace_config_sig.read());
        }
    });
    let missed_surface_strategy_handler =
        EventHandler::new(move |missed_surface_strategy: MissedSurfaceStrategy| {
            ray_trace_config_sig
                .write()
                .set_missed_surface_strategy(missed_surface_strategy);
            ray_trace_config_handler.call(*ray_trace_config_sig.read());
        });

    rsx! {
        FlushableTextInput {
            id: "rayTraceMaxRefr".to_string(),
            label: "Max refractions".to_string(),
            value: format!("{}", ray_trace_config_sig.read().max_number_of_refractions()),
            r#type: "number",
            step: "1",
            min: "0",
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
            on_save: move |val: String| {
                if let Ok(max_refractions) = val.parse::<usize>() {
                    max_refractions_handler.call(max_refractions);
                }
            },
        }
        FlushableTextInput {
            id: "rayTraceMaxBounces".to_string(),
            label: "Max bounces".to_string(),
            value: format!("{}", ray_trace_config_sig.read().max_number_of_bounces()),
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
        NodeConfigUnitInput {
            id: "rayTraceMinEnergy".to_string(),
            label: "Minimum ray energy".to_string(),
            value: ray_trace_config_sig.read().min_energy_per_ray().value,
            base_unit: "J",
            onchange: move |val: f64| {
                if val >= 0.0 {
                    min_ray_energy_handler.call(joule!(val));
                } else {
                    OPOSSUM_UI_LOGS.write().add_log("Minimum ray energy must be non-negative.");
                }
            },
        }
        // FlushableTextInput {
        //     id: "rayTraceMinEnergy".to_string(),
        //     label: "Minimum ray energy in pJ".to_string(),
        //     value: format!("{}", ray_trace_config_sig.read().min_energy_per_ray().get::<picojoule>()),
        //     r#type: "number",
        //     step: "1.",
        //     min: "0.",
        //     container_class: "form-floating border-start".to_string(),
        //     input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
        //     label_class: "form-label text-secondary".to_string(),
        //     on_save: move |val: String| {
        //         let old_value = ray_trace_config_sig.read().min_energy_per_ray();
        //         if let Ok(min_ray_energy) = val.parse::<f64>() {
        //             if min_ray_energy < 0.0 {
        //                 OPOSSUM_UI_LOGS
        //                     .write()
        //                     .add_log("Minimum ray energy must be non-negative.");
        //                 ray_trace_config_sig
        //                     .write()
        //                     .set_min_energy_per_ray(old_value)
        //                     .unwrap_or_else(|err| {
        //                         OPOSSUM_UI_LOGS.write().add_log(&err.to_string());
        //                     });
        //             } else {
        //                 let update_result = ray_trace_config_sig
        //                     .write()
        //                     .set_min_energy_per_ray(picojoule!(min_ray_energy));

        //                 match update_result {
        //                     Ok(()) => {
        //                         on_change
        //                             .call(NodeChangeEvent {
        //                                 node_id: *node_id.read(),
        //                                 action: NodeChangeAction::AnalyzerType(
        //                                     AnalyzerType::RayTrace(*ray_trace_config_sig.read()),
        //                                 ),
        //                             });
        //                     }
        //                     Err(err) => OPOSSUM_UI_LOGS.write().add_log(&err.to_string()),
        //                 }
        //             }
        //         }
        //     },
        // }
        // Selects brauchen kein Flushable, da sie sofort feuern
        LabeledSelect {
            id: "rayTraceMissedSurf".to_string(),
            label: "Missed-Surface Strategy".to_string(),
            options: select_options_from_enum_iterator(
                ray_trace_config_sig.read().missed_surface_strategy(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(surface_strategy) = MissedSurfaceStrategy::default_from_name(
                    val.as_str(),
                ) {
                    missed_surface_strategy_handler.call(surface_strategy);
                }
            },
        }
    }
}
