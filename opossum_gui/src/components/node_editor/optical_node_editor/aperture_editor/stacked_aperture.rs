use dioxus::prelude::*;
use opossum_core::{
    apertures::StackShape,
    prelude::{Aperture, ApertureShape, ApertureType},
    utils::default_from_name::DefaultFromName,
};

use crate::{
    OPOSSUM_UI_LOGS,
    components::{
        logger::LogResultExt,
        node_editor::{
            inputs::{
                dynamic_list::DynamicListComponent,
                input_components::{LabeledSelect, RowedInputs},
                select_options_from_enum_iterator,
            },
            optical_node_editor::aperture_editor::{
                PolygonApertureInput, aperture_component::get_aperture_input_data,
            },
        },
    },
};
#[component]
pub fn StackedApertureInput(
    stacked_aperture: StackShape,
    on_shape_change: EventHandler<ApertureShape>,
    readonly: bool,
) -> Element {
    let mut current_aperture = use_signal(Aperture::default);
    let mut stacked_aperture_sig = use_signal(|| stacked_aperture.clone());
    let mut editing_index = use_signal(|| None::<usize>);

    let on_save = EventHandler::new(move |stack: StackShape| {
        if stack != *stacked_aperture_sig.read() {
            on_shape_change.call(ApertureShape::Stack(stacked_aperture_sig.read().clone()));
            stacked_aperture_sig.set(stack);
        }
    });

    let on_current_aperture_change = EventHandler::new(move |aperture: Aperture| {
        if aperture != *current_aperture.read() {
            current_aperture.set(aperture);
        }
    });

    let on_current_aperture_shape_change = EventHandler::new(move |aperture: ApertureShape| {
        let ap_shape = current_aperture.read().shape().clone();
        if aperture != ap_shape {
            current_aperture.write().set_shape(aperture);
        }
    });

    let on_current_aperture_type_change = EventHandler::new(move |aperture_type: ApertureType| {
        let ap_type = *current_aperture.read().aperture_type();
        if aperture_type != ap_type {
            current_aperture.write().set_aperture_type(aperture_type);
        }
    });

    let on_editing_index_change = EventHandler::new(move |index_opt: Option<usize>| {
        if index_opt != *editing_index.read() {
            editing_index.set(index_opt);
        }
    });

    rsx! {
        LabeledSelect {
            id: "apertureTypeSelectorStack",
            label: "Aperture type",
            options: select_options_from_enum_iterator(current_aperture.read().aperture_type(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(aperture_type) = ApertureType::default_from_name(val.as_str()) {
                    on_current_aperture_type_change.call(aperture_type);
                }
            },
        }
        LabeledSelect {
            id: "apertureShapeSelectorStack",
            label: "Select Aperture for Stack",
            options: select_options_from_enum_iterator(
                current_aperture.read().shape(),
                Some(&[&ApertureShape::Stack(StackShape::default())]),
            ),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(aperture_shape) = ApertureShape::default_from_name(val.as_str()) {
                    on_current_aperture_shape_change.call(aperture_shape);
                }
            },
        }

        div { class: "accordion-content-wrapper-div border-start",
            {
                if let ApertureShape::BinaryPolygon(polygon_config) = current_aperture
                    .read()
                    .shape()
                {
                    rsx! {
                        PolygonApertureInput {
                            polygon_config: polygon_config.clone(),
                            on_shape_change: on_current_aperture_shape_change,
                            readonly,
                        }
                    }
                } else {
                    rsx! {
                        RowedInputs {
                            inputs: get_aperture_input_data(
                                current_aperture.read().shape(),
                                on_current_aperture_shape_change,
                                readonly,
                            ),
                        }
                    }
                }
            }
        }
        input {
            class: " border-start btn",
            // r#type: "submit",
            id: "stackedApertureSubmit",
            value: if editing_index.read().is_none() {
                "Add Aperture to Stack"
            } else {
                "Update Aperture in Stack"
            },
            readonly,
            disabled: readonly,
            onclick: {
                let mut stacked_aperture = stacked_aperture_sig.read().clone();
                let current_aperture = current_aperture.read().clone();
                move |_| {
                    if !readonly {
                        let edit_index = *editing_index.read();
                        if let Some(i) = edit_index {
                            stacked_aperture
                                // match stacked_aperture.add_aperture(current_aperture.clone()) {
                                //     Ok(()) => on_save.call(stacked_aperture.clone()),
                                //     Err(e) => {
                                //         OPOSSUM_UI_LOGS
                                //             .write()
                                //             .add_log(
                                //                 format!("Error adding aperture to stack: {e}").as_str(),
                                //             )
                                //     }
                                // }
                                .set_aperture(i, current_aperture.clone())
                                .log_err_with_context("Updating aperture in stack failed");
                            on_editing_index_change.call(None);
                            on_save.call(stacked_aperture.clone());
                        } else {
                            match stacked_aperture.add_aperture(current_aperture.clone()) {
                                Ok(()) => on_save.call(stacked_aperture.clone()),
                                Err(e) => {
                                    OPOSSUM_UI_LOGS
                                        .write()
                                        .add_log(
                                            format!("Error adding aperture to stack: {e}").as_str(),
                                        );
                                }
                            }
                        }
                    }
                }
            },
        }
        StackedAperturesList {
            stacked_aperture: stacked_aperture_sig.read().clone(),
            on_save,
            on_current_aperture_change,
            on_editing_index_change,
            readonly,
            editing_index,
        }

    }
}

#[component]
fn StackedAperturesList(
    on_save: EventHandler<StackShape>,
    stacked_aperture: StackShape,
    on_current_aperture_change: EventHandler<Aperture>,
    on_editing_index_change: EventHandler<Option<usize>>,
    readonly: bool,
    editing_index: ReadSignal<Option<usize>>,
) -> Element {
    let list_entries = stacked_aperture
        .apertures()
        .iter()
        .map(|ap| {
            vec![
                format!("{}", ap.shape()),
                format!("Type: {}", ap.aperture_type()),
            ]
        })
        .collect::<Vec<Vec<String>>>();

    let delete_entry_handler = EventHandler::new({
        let stacked_aperture = stacked_aperture.clone();
        move |index: usize| {
            let mut stacked_aperture = stacked_aperture.clone();
            if stacked_aperture.delete_aperture(index).is_ok() {
                on_save.call(stacked_aperture);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(format!("Error deleting aperture at index {index}").as_str());
            }
        }
    });

    let modify_entry_handler = EventHandler::new(move |index: usize| {
        if let Ok(aperture) = stacked_aperture.get_aperture(index).cloned() {
            on_current_aperture_change.call(aperture);
            on_editing_index_change.call(Some(index));
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(format!("Error retrieving aperture at index {index}").as_str());
        }
    });

    rsx! {
        DynamicListComponent {
            list_entries,
            delete_entry_handler,
            modify_entry_handler,
            edit_index: editing_index,
            readonly,
        }
    }
}
