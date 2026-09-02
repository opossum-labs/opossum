//! Editor for the [`PumpSource`] of one node within one pump scenario.
//!
//! The counterpart of the gain switch next to it: that one says what a beam makes of the medium,
//! this one says how the medium got its inversion in the first place. Both belong to the scenario
//! rather than to the node, so everything here edits one node *within one operating point*.
//!
//! Every widget below hands its parent a **complete** [`PumpSource`], rebuilt from the value
//! currently shown plus the one thing the user just changed. Nothing keeps a half-edited pump source
//! around, so no state can drift out of sync with what the document holds — and a value the core
//! refuses simply never leaves this module.

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        hooks::use_synced_signal,
        inputs::{
            input_components::{
                FormContext, LabeledCheckboxInput, LabeledSelect, NodeConfigPlainF64Input,
                NodeConfigUnitInput, NodeConfigUsizeInput, UnitHandling,
            },
            select_options_from_enum_iterator,
        },
    },
};
use dioxus::prelude::*;
use opossum_core::{
    degree,
    gain::{
        AnalyticPump, LambertBeerProfile, LongitudinalProfile, PumpDirection, PumpSource,
        TransversalProfile,
    },
    meter, reciprocal_meter,
    utils::{default_from_name::DefaultFromName, super_gaussian::SuperGaussianShape},
};
use uom::si::angle::degree;

/// The pump source of one node in one scenario: a variant dropdown plus, per variant, its
/// parameters.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `source` - the pump source as the document currently holds it.
/// * `on_change` - handed the complete new pump source whenever the user changes anything.
#[component]
pub fn PumpSourceEditor(
    id_prefix: String,
    source: PumpSource,
    on_change: EventHandler<PumpSource>,
) -> Element {
    let flush_trigger = use_signal(|| 0usize);
    let dirty_count = use_signal(|| 0usize);
    use_context_provider(|| FormContext {
        flush_trigger,
        dirty_count,
    });

    rsx! {
        div { class: "amp-config-block",
            LabeledSelect {
                id: format!("{id_prefix}-pump"),
                label: "Pump".to_string(),
                options: select_options_from_enum_iterator(&source, None),
                onchange: move |e: Event<FormData>| {
                    // A freshly picked variant starts at its own default, which pumps nothing until
                    // its parameters are filled in - the same discipline the gain switch follows.
                    if let Some(picked) = PumpSource::default_from_name(&e.value()) {
                        on_change.call(picked);
                    }
                },
            }
            match source {
                // A uniform pump carries no parameters of its own now - how hard it pumps is the
                // gain coefficient on the model, edited next to this. So it shows nothing here, like
                // an absent pump does.
                PumpSource::Analytic(analytic) => rsx! {
                    AnalyticPumpFields { id_prefix, analytic, on_change }
                },
                _ => rsx! {},
            }
        }
    }
}

/// The parameters of an [`AnalyticPump`]: one dropdown per profile it composes, plus the grid the
/// shape is resolved on. Its magnitude is not here — that is the gain coefficient on the model.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `analytic` - the pump as the document currently holds it.
/// * `on_change` - handed the complete new pump source whenever the user changes anything.
#[allow(clippy::cast_possible_truncation)]
#[component]
fn AnalyticPumpFields(
    id_prefix: String,
    analytic: AnalyticPump,
    on_change: EventHandler<PumpSource>,
) -> Element {
    // Rebuilding the whole pump from its parts is what every edit below does, so it is spelled out
    // once here. Only a zero-cell grid can make it fail.
    let rebuilt = move |transversal, longitudinal, grid| {
        save(
            AnalyticPump::new(transversal, longitudinal, grid).map(PumpSource::Analytic),
            on_change,
        );
    };
    let (transversal, longitudinal) = (analytic.transversal(), analytic.longitudinal());
    let grid = analytic.grid();
    rsx! {
        LabeledSelect {
            id: format!("{id_prefix}-pump-transversal"),
            label: "Transversal profile".to_string(),
            options: select_options_from_enum_iterator(&transversal, None),
            onchange: move |e: Event<FormData>| {
                if let Some(picked) = TransversalProfile::default_from_name(&e.value()) {
                    rebuilt(picked, longitudinal, grid);
                }
            },
        }
        if let TransversalProfile::SuperGaussian(shape) = transversal {
            SuperGaussianFields {
                id_prefix: id_prefix.clone(),
                shape,
                on_change: move |shaped| rebuilt(TransversalProfile::SuperGaussian(shaped), longitudinal, grid),
            }
        }
        LabeledSelect {
            id: format!("{id_prefix}-pump-longitudinal"),
            label: "Longitudinal profile".to_string(),
            options: select_options_from_enum_iterator(&longitudinal, None),
            onchange: move |e: Event<FormData>| {
                if let Some(picked) = LongitudinalProfile::default_from_name(&e.value()) {
                    rebuilt(transversal, picked, grid);
                }
            },
        }
        if let LongitudinalProfile::LambertBeer(profile) = longitudinal {
            LambertBeerFields {
                id_prefix: id_prefix.clone(),
                profile,
                on_change: move |absorbed| rebuilt(
                    transversal,
                    LongitudinalProfile::LambertBeer(absorbed),
                    grid,
                ),
            }
        }
        div { class: "amp-pump-nested",
            div { class: "ssg-params-grid",
                NodeConfigUsizeInput {
                    id: format!("{id_prefix}-pump-cells-x"),
                    label: "Cells x".to_string(),
                    value: grid.0,
                    onchange: move |v| rebuilt(transversal, longitudinal, (v, grid.1, grid.2)),
                }
                NodeConfigUsizeInput {
                    id: format!("{id_prefix}-pump-cells-y"),
                    label: "Cells y".to_string(),
                    value: grid.1,
                    onchange: move |v| rebuilt(transversal, longitudinal, (grid.0, v, grid.2)),
                }
                NodeConfigUsizeInput {
                    id: format!("{id_prefix}-pump-cells-z"),
                    label: "Cells z".to_string(),
                    value: grid.2,
                    onchange: move |v| rebuilt(transversal, longitudinal, (grid.0, grid.1, v)),
                }
            }
        }
    }
}

/// The parameters of a [`SuperGaussianShape`]: where the pump spot sits, how wide it is, how steep
/// its flanks are and how it is turned.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `shape` - the spot as the document currently holds it.
/// * `on_change` - handed the complete new shape whenever the user changes anything.
#[component]
fn SuperGaussianFields(
    id_prefix: String,
    shape: SuperGaussianShape,
    on_change: EventHandler<SuperGaussianShape>,
) -> Element {
    // Every field below rebuilds the whole shape, so a value the core refuses - a width of zero,
    // say - leaves the previous one standing rather than being half applied.
    let rebuilt = move |center, sigma, power, theta, rectangular| {
        save(
            SuperGaussianShape::new(center, sigma, power, theta, rectangular),
            on_change,
        );
    };
    let (center, sigma) = (shape.center(), shape.sigma());
    let (power, theta, rectangular) = (shape.power(), shape.theta(), shape.rectangular());

    // Synced signals drive NodeConfigUnitInput's reactive re-sync when the shape is changed
    // externally (a save completing, an undo) without losing a mid-edit value.
    let sig_sigma_x = use_synced_signal(sigma.x);
    let sig_sigma_y = use_synced_signal(sigma.y);
    let sig_center_x = use_synced_signal(center.x);
    let sig_center_y = use_synced_signal(center.y);
    let sig_theta = use_synced_signal(theta);
    let sig_power = use_synced_signal(power);

    let unit_m = UnitHandling::new("m", true);

    rsx! {
        div { class: "amp-pump-nested",
            div { class: "field-pair",
                NodeConfigUnitInput {
                    id: format!("{id_prefix}-spot-sigma-x"),
                    label: "σx",
                    value: sig_sigma_x.read().value,
                    unit_config: unit_m.clone(),
                    onchange: move |x_m: f64| {
                        rebuilt(
                            center,
                            meter!(x_m, sigma.y.get::< uom::si::length::meter > ()),
                            power,
                            theta,
                            rectangular,
                        );
                    },
                }
                NodeConfigUnitInput {
                    id: format!("{id_prefix}-spot-sigma-y"),
                    label: "σy",
                    value: sig_sigma_y.read().value,
                    unit_config: unit_m.clone(),
                    onchange: move |y_m: f64| {
                        rebuilt(
                            center,
                            meter!(sigma.x.get::< uom::si::length::meter > (), y_m),
                            power,
                            theta,
                            rectangular,
                        );
                    },
                }
            }
            div { class: "field-pair",
                NodeConfigUnitInput {
                    id: format!("{id_prefix}-spot-mu-x"),
                    label: "Center x",
                    value: sig_center_x.read().value,
                    unit_config: unit_m.clone(),
                    onchange: move |x_m: f64| {
                        rebuilt(
                            meter!(x_m, center.y.get::< uom::si::length::meter > ()),
                            sigma,
                            power,
                            theta,
                            rectangular,
                        );
                    },
                }
                NodeConfigUnitInput {
                    id: format!("{id_prefix}-spot-mu-y"),
                    label: "Center y",
                    value: sig_center_y.read().value,
                    unit_config: unit_m,
                    onchange: move |y_m: f64| {
                        rebuilt(
                            meter!(center.x.get::< uom::si::length::meter > (), y_m),
                            sigma,
                            power,
                            theta,
                            rectangular,
                        );
                    },
                }
            }
            NodeConfigPlainF64Input {
                id: format!("{id_prefix}-spot-power"),
                label: "Order (1 = Gaussian)".to_string(),
                value: sig_power,
                onchange: move |value: f64| {
                    rebuilt(center, sigma, value, theta, rectangular);
                },
            }
            NodeConfigUnitInput {
                id: format!("{id_prefix}-spot-theta"),
                label: "Rotation",
                value: sig_theta.read().get::<degree>(),
                unit_config: UnitHandling::new("°", true),
                onchange: move |d: f64| {
                    rebuilt(center, sigma, power, degree!(d), rectangular);
                },
            }
            LabeledCheckboxInput {
                id: format!("{id_prefix}-spot-rectangular"),
                label: "Rectangular flanks".to_string(),
                value: format!("{rectangular}"),
                onchange: move |e: Event<FormData>| {
                    rebuilt(center, sigma, power, theta, e.checked());
                },
            }
        }
    }
}

/// The parameters of a [`LambertBeerProfile`]: how strongly the pump is absorbed, and which face it
/// enters through.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `profile` - the absorption as the document currently holds it.
/// * `on_change` - handed the complete new profile whenever the user changes anything.
#[component]
fn LambertBeerFields(
    id_prefix: String,
    profile: LambertBeerProfile,
    on_change: EventHandler<LambertBeerProfile>,
) -> Element {
    let (absorption, direction) = (profile.absorption(), profile.direction());
    let sig_absorption = use_synced_signal(absorption);
    rsx! {
        div { class: "amp-pump-nested",
            NodeConfigUnitInput {
                id: format!("{id_prefix}-beer-alpha"),
                label: "Absorption α",
                value: sig_absorption.read().value,
                unit_config: UnitHandling::new("m⁻¹", true),
                reciprocal: true,
                onchange: move |v: f64| {
                    save(LambertBeerProfile::new(reciprocal_meter!(v), direction), on_change);
                },
            }
            LabeledSelect {
                id: format!("{id_prefix}-beer-direction"),
                label: "Pumped from".to_string(),
                options: select_options_from_enum_iterator(&direction, None),
                onchange: move |e: Event<FormData>| {
                    if let Some(picked) = PumpDirection::default_from_name(&e.value()) {
                        save(LambertBeerProfile::new(absorption, picked), on_change);
                    }
                },
            }
        }
    }
}

/// Push a rebuilt value upward, or report why the core refused it.
///
/// Every widget in this module rebuilds its whole value rather than patching one field of it, so
/// this is the single place a rejected edit is turned into a log line instead of a broken value.
///
/// # Arguments
///
/// * `rebuilt` - what the core made of the user's edit.
/// * `on_change` - the parent to hand the result to, if there is one to hand over.
fn save<T: 'static>(rebuilt: opossum_core::error::OpmResult<T>, on_change: EventHandler<T>) {
    match rebuilt {
        Ok(value) => on_change.call(value),
        Err(err) => OPOSSUM_UI_LOGS
            .write()
            .add_log(&format!("cannot apply that pump setting: {err}")),
    }
}
