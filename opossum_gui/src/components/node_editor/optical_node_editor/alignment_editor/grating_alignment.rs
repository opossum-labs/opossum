#![allow(clippy::derive_partial_eq_without_eq)]
use std::vec;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        inputs::input_components::{LabeledSelect, NodeConfigUnitInput},
        optical_node_editor::alignment_editor::{
            RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation,
            on_new_translation,
        },
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use opossum_core::{
    degree, meter, nanometer, num_per_mm,
    prelude::{Isometry, Properties, Proptype},
    radian,
    utils::{geom_transformation::RotationAxis, to_f64},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Length, LinearNumberDensity},
    length::meter,
    linear_number_density::per_meter,
};
use uuid::Uuid;

#[component]
pub fn GratingAlignmentInputs(
    alignment_sig_outside: ReadSignal<Isometry>,
    node_properties: ReadSignal<Properties>,
    on_save: EventHandler<Isometry>,
    node_id: Memo<Uuid>,
) -> Element {
    let mut alignment_select_sig = use_signal(|| true);
    let alignment_memo = use_memo(move || *alignment_sig_outside.read());
    let diffraction_order_memo = use_memo(move || {
        if let Ok(Proptype::I32(p)) = node_properties.read().get("diffraction order") {
            *p
        } else {
            -1
        }
    });
    let line_density_memo = use_memo(move || {
        if let Ok(Proptype::LinearDensity(p)) = node_properties.read().get("line density") {
            *p
        } else {
            num_per_mm!(1740.)
        }
    });

    let mut element_list = vec![rsx! {
        GratingAlignmentSelector { alignment_select_sig, on_alignment_change: move |new_state| alignment_select_sig.set(new_state) }
    }];

    if let (true, Ok(Proptype::I32(_)), Ok(Proptype::LinearDensity(_))) = (
        *alignment_select_sig.read(),
        node_properties.read().get("diffraction order").cloned(),
        node_properties.read().get("line density").cloned(),
    ) {
        element_list.push(rsx! {
            LittrowConfigEditor {
                alignment_memo,
                diffraction_order_memo,
                line_density_memo,
                on_alignment_change: move |iso: Isometry| {
                    on_save.call(iso);
                },
            }
        });

        element_list.push(rsx! {
            RotationAlignmentInputs {
                alignment: alignment_memo,
                axes_skip: Some(vec![RotationAxis::Pitch]),
                on_new_rotation: on_new_rotation(on_save, alignment_memo.into()),
                node_id,
            }
            TranslationAlignmentInputs {
                alignment: alignment_memo,
                on_new_translation: on_new_translation(on_save, alignment_memo.into()),
                node_id,
            }
        });
    } else {
        element_list.push(rsx! {

            RotationAlignmentInputs {
                alignment: alignment_memo,
                axes_skip: None,
                on_new_rotation: on_new_rotation(on_save, alignment_memo.into()),
                node_id,

            }
            TranslationAlignmentInputs {
                alignment: alignment_memo,
                on_new_translation: on_new_translation(on_save, alignment_memo.into()),
                node_id,
            }
        });
    }
    rsx! {
        for element in element_list {
            {element}
        }
    }
}

#[component]
pub fn LittrowConfigEditor(
    alignment_memo: ReadSignal<Isometry>,
    diffraction_order_memo: Memo<i32>,
    line_density_memo: Memo<LinearNumberDensity>,
    on_alignment_change: EventHandler<Isometry>,
) -> Element {
    let mut incident_angle_sig = use_signal(|| true);
    let mut reference_wavelength_sig = use_signal(|| nanometer!(1053.));
    rsx! {
        InOrOutgoingFromLittrowSelector {
            incident_angle_sig,
            on_incident_change: move |new_state: bool| {
                incident_angle_sig.set(new_state);
            },
        }
        NodeConfigUnitInput {
            id: "alignmentWavelengthGrating",
            label: "Reference wavelength",
            value: reference_wavelength_sig.read().value,
            base_unit: "m",
            onchange: move |new_length: f64| {
                if relative_ne!(reference_wavelength_sig.read().value, new_length) {
                    reference_wavelength_sig.set(meter!(new_length));
                }
            },
        }
        AngleToLittrowComponent {
            incident_angle_sig,
            reference_wavelength_sig,
            diffraction_order_memo,
            line_density_memo,
            alignment_memo,
            on_alignment_change,
        }
    }
}

fn calc_deviation_angle_from_littrow(
    diffraction_order: i32,
    line_density: LinearNumberDensity,
    rotation_angle: Angle,
    alignment_wavelength: Length,
    incident_angle: bool,
) -> Angle {
    let sin_theta: f64 =
        to_f64(diffraction_order) * alignment_wavelength.value * line_density.value;
    let littrow_angle = radian!((sin_theta / 2.).asin());

    if incident_angle {
        rotation_angle - littrow_angle
    } else {
        radian!((-rotation_angle.get::<radian>().sin() + sin_theta).asin()) - littrow_angle
    }
}

#[component]
fn AngleToLittrowComponent(
    incident_angle_sig: ReadSignal<bool>,
    reference_wavelength_sig: ReadSignal<Length>,
    diffraction_order_memo: Memo<i32>,
    line_density_memo: Memo<LinearNumberDensity>,
    alignment_memo: ReadSignal<Isometry>,
    on_alignment_change: EventHandler<Isometry>,
) -> Element {
    let angle_from_littrow = use_memo(move || {
        calc_deviation_angle_from_littrow(
            *diffraction_order_memo.read(),
            *line_density_memo.read(),
            alignment_memo.read().rotation_of_axis(RotationAxis::Pitch),
            *reference_wavelength_sig.read(),
            *incident_angle_sig.read(),
        )
        .get::<degree>()
    });
    rsx! {
        NodeConfigUnitInput {
            id: "angleToLittrowGrating",
            label: "Angle",
            value: angle_from_littrow,
            base_unit: "°",
            onchange: move |angle: f64| {
                let m_g_lambda = reference_wavelength_sig.read().get::<meter>()
                    * line_density_memo.read().get::<per_meter>()
                    * to_f64(*diffraction_order_memo.read());
                let littrow_angle = (m_g_lambda / 2.).asin();
                let mut new_angle = radian!(littrow_angle) + degree!(angle);
                if !*incident_angle_sig.read() {
                    new_angle = radian!((- new_angle.sin().value + m_g_lambda).asin());
                }
                let mut iso = *alignment_memo.read();
                let update_res = iso.set_rotation_of_axis(RotationAxis::Pitch, new_angle);
                match update_res {
                    Ok(()) => {
                        on_alignment_change.call(iso);
                    }
                    Err(e) => {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log(&format!("Failed to set rotation of isometry: {e}"));
                    }
                }
            },
        }
        // LabeledInput {
        //     id: "angleToLittrowGrating",
        //     label: "Angle in degrees",
        //     value: format!(
        //         "{:.3}",
        //         calc_deviation_angle_from_littrow(
        //                 diffraction_order,
        //                 line_density,
        //                 alignment_memo.read().rotation_of_axis(RotationAxis::Pitch),
        //                 *reference_wavelength_sig.read(),
        //                 *incident_angle_sig.read(),
        //             )
        //             .get::<degree>(),
        //     ),
        //     r#type: "number",
        //     step: Some("0.01"),
        //     onchange: move |e: Event<FormData>| {
        //         if let Ok(angle) = e.data.value().parse::<f64>() {
        //             let m_g_lambda = reference_wavelength_sig.read().get::<meter>()
        //                 * line_density.get::<per_meter>() * to_f64(diffraction_order);
        //             let littrow_angle = (m_g_lambda / 2.).asin();
        //             let mut new_angle = radian!(littrow_angle) + degree!(angle);
        //             if !*incident_angle_sig.read() {
        //                 new_angle = radian!((- new_angle.sin().value + m_g_lambda).asin());
        //             }

        //             let mut iso = *alignment_memo.read();
        //             let update_res = iso.set_rotation_of_axis(RotationAxis::Pitch, new_angle);

        //             match update_res {
        //                 Ok(()) => {
        //                     on_alignment_change.call(iso);
        //                 }
        //                 Err(e) => {
        //                     OPOSSUM_UI_LOGS
        //                         .write()
        //                         .add_log(&format!("Failed to set rotation of isometry: {e}"));
        //                 }
        //             }
        //         }
        //     },
        // }
    }
}

#[component]
pub fn GratingAlignmentSelector(
    alignment_select_sig: ReadSignal<bool>,
    on_alignment_change: EventHandler<bool>,
) -> Element {
    let via_littrow = "Define pitch via littrow";
    let direct_pitch = "Define pitch directly";
    rsx! {
        LabeledSelect {
            id: "gratingAlignmentSelection",
            label: "Alignment config.",
            options: vec![
                (*alignment_select_sig.read(), via_littrow.to_string()),
                (!*alignment_select_sig.read(), direct_pitch.to_string()),
            ],
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if val.as_str() == via_littrow {
                    on_alignment_change.call(true);
                } else {
                    on_alignment_change.call(false);
                }
            },
        }
    }
}

#[component]
pub fn InOrOutgoingFromLittrowSelector(
    incident_angle_sig: ReadSignal<bool>,
    on_incident_change: EventHandler<bool>,
) -> Element {
    let incident_label = "Incident angle to Littrow";
    let diffracted_label = "Diffracted angle to Littrow";

    rsx! {
        LabeledSelect {
            id: "fromOrToLittrowSelection",
            label: "Alignment config.",
            options: vec![
                (*incident_angle_sig.read(), incident_label.to_string()),
                (!*incident_angle_sig.read(), diffracted_label.to_string()),
            ],
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if val.as_str() == incident_label {
                    on_incident_change.call(true);
                } else {
                    on_incident_change.call(false);
                }
            },
        }
    }
}
