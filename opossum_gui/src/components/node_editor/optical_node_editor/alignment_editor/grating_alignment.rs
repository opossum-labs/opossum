#![allow(clippy::derive_partial_eq_without_eq)]
use std::vec;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        inputs::input_components::{LabeledInput, LabeledSelect},
        optical_node_editor::alignment_editor::{
            RotationAlignmentInputs, TranslationAlignmentInputs,
        },
    },
};
use dioxus::prelude::*;
use opossum_core::{
    degree, nanometer,
    prelude::{Isometry, Properties, Proptype},
    radian,
    utils::{geom_transformation::RotationAxis, to_f64},
};
use uom::si::{
    angle::degree,
    f64::{Angle, Length, LinearNumberDensity},
    length::{meter, nanometer},
    linear_number_density::per_meter,
};
use uuid::Uuid;

#[component]
pub fn GratingAlignmentInputs(
    alignment_sig: Signal<Isometry>,
    node_properties_sig: Signal<Properties>,
    on_save: EventHandler<Isometry>,
    node_id: Uuid,
) -> Element {
    let alignment_select_sig = use_signal(|| true);

    let mut element_list = vec![rsx! {
        GratingAlignmentSelector { alignment_select_sig }
    }];

    if let (true, Ok(Proptype::I32(diffraction_order)), Ok(Proptype::LinearDensity(line_density))) = (
        *alignment_select_sig.read(),
        node_properties_sig.read().get("diffraction order").cloned(),
        node_properties_sig.read().get("line density").cloned(),
    ) {
        element_list.push(rsx! {
            LittrowConfigEditor {
                alignment_sig,
                diffraction_order,
                line_density,
                on_save,
            }
        });
        element_list.push(rsx! {});
    } else {
        element_list.push(rsx! {});
    }
    rsx! {
        for element in element_list {
            {element}
        }
    }
}

#[component]
pub fn LittrowConfigEditor(
    alignment_sig: Signal<Isometry>,
    diffraction_order: i32,
    line_density: LinearNumberDensity,
    on_save: EventHandler<Isometry>,
) -> Element {
    let incident_angle_sig = use_signal(|| true);
    let mut reference_wavelength_sig = use_signal(|| nanometer!(1053.));
    rsx! {
        InOrOutgoingFromLittrowSelector { incident_angle_sig }
        LabeledInput {
            id: "alignmentWavelengthGrating",
            label: "Reference wavelength",
            value: format!("{:.3}", reference_wavelength_sig.read().get::<nanometer>()),
            r#type: "number",
            onchange: move |e: Event<FormData>| {
                if let Ok(length) = e.data.value().parse::<f64>() {
                    reference_wavelength_sig.set(nanometer!(length));
                }
            },
        }
        AngleToLittrowComponent {
            incident_angle_sig,
            reference_wavelength_sig,
            diffraction_order,
            line_density,
            alignment_sig,
            on_save,
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
    incident_angle_sig: Signal<bool>,
    reference_wavelength_sig: Signal<Length>,
    diffraction_order: i32,
    line_density: LinearNumberDensity,
    mut alignment_sig: Signal<Isometry>,
    on_save: EventHandler<Isometry>,
) -> Element {
    rsx! {
        LabeledInput {
            id: "angleToLittrowGrating",
            label: "Angle in degrees",
            value: format!(
                "{:.3}",
                calc_deviation_angle_from_littrow(
                        diffraction_order,
                        line_density,
                        alignment_sig.read().rotation_of_axis(RotationAxis::Pitch),
                        *reference_wavelength_sig.read(),
                        *incident_angle_sig.read(),
                    )
                    .get::<degree>(),
            ),
            r#type: "number",
            step: Some("0.01"),
            onchange: move |e: Event<FormData>| {
                if let Ok(angle) = e.data.value().parse::<f64>() {
                    let m_g_lambda = reference_wavelength_sig.read().get::<meter>()
                        * line_density.get::<per_meter>() * to_f64(diffraction_order);
                    let littrow_angle = (m_g_lambda / 2.).asin();
                    let mut new_angle = radian!(littrow_angle) + degree!(angle);
                    if !*incident_angle_sig.read() {
                        new_angle = radian!((- new_angle.sin().value + m_g_lambda).asin());
                    }

                    let mut iso = *alignment_sig.read();
                    let update_res = iso.set_rotation_of_axis(RotationAxis::Pitch, new_angle);

                    match update_res {
                        Ok(()) => {
                            alignment_sig.set(iso);
                            on_save.call(iso);
                        }
                        Err(e) => {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("Failed to set rotation of isometry: {e}"));
                        }
                    }
                }
            },
        }
    }
}

#[component]
pub fn GratingAlignmentSelector(mut alignment_select_sig: Signal<bool>) -> Element {
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
                    alignment_select_sig.set(true);
                } else {
                    alignment_select_sig.set(false);
                }
            },
        }
    }
}

#[component]
pub fn InOrOutgoingFromLittrowSelector(incident_angle_sig: Signal<bool>) -> Element {
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
                    incident_angle_sig.set(true);
                } else {
                    incident_angle_sig.set(false);
                }
            },
        }
    }
}
