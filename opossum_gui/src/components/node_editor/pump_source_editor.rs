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
    components::node_editor::inputs::{
        input_components::LabeledSelect, select_options_from_enum_iterator,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    degree,
    gain::{
        AnalyticPump, BeerLambertProfile, ConstInversion, LongitudinalProfile, PumpDirection,
        PumpSource, TransversalProfile,
    },
    millimeter, reciprocal_centimeter,
    utils::{default_from_name::DefaultFromName, super_gaussian::SuperGaussianShape},
};
use uom::si::{
    angle::degree, f64::Length, length::millimeter, reciprocal_length::reciprocal_centimeter,
};

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
                PumpSource::Const(constant) => rsx! {
                    NumberField {
                        id: format!("{id_prefix}-pump-g0"),
                        label: "Gain g₀ (1/cm)".to_string(),
                        value: constant.gain_coefficient().get::<reciprocal_centimeter>(),
                        step: 0.1,
                        min: None,
                        on_save: move |value: f64| {
                            save(
                                ConstInversion::new(reciprocal_centimeter!(value)).map(PumpSource::Const),
                                on_change,
                            );
                        },
                    }
                },
                PumpSource::Analytic(analytic) => rsx! {
                    AnalyticPumpFields { id_prefix, analytic, on_change }
                },
                _ => rsx! {},
            }
        }
    }
}

/// The parameters of an [`AnalyticPump`]: its peak, and one dropdown per profile it composes.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `analytic` - the pump as the document currently holds it.
/// * `on_change` - handed the complete new pump source whenever the user changes anything.
#[component]
fn AnalyticPumpFields(
    id_prefix: String,
    analytic: AnalyticPump,
    on_change: EventHandler<PumpSource>,
) -> Element {
    // Rebuilding the whole pump from its three parts is what every edit below does, so it is spelled
    // out once here. The peak is already validated, so only a rejected *new* peak can fail.
    let rebuilt = move |peak, transversal, longitudinal| {
        save(
            AnalyticPump::new(peak, transversal, longitudinal).map(PumpSource::Analytic),
            on_change,
        );
    };
    let peak = analytic.peak_gain_coefficient();
    let (transversal, longitudinal) = (analytic.transversal(), analytic.longitudinal());
    rsx! {
        NumberField {
            id: format!("{id_prefix}-pump-peak"),
            label: "Peak gain g₀ (1/cm)".to_string(),
            value: peak.get::<reciprocal_centimeter>(),
            step: 0.1,
            min: None,
            on_save: move |value: f64| {
                rebuilt(reciprocal_centimeter!(value), transversal, longitudinal);
            },
        }
        LabeledSelect {
            id: format!("{id_prefix}-pump-transversal"),
            label: "Transversal profile".to_string(),
            options: select_options_from_enum_iterator(&transversal, None),
            onchange: move |e: Event<FormData>| {
                if let Some(picked) = TransversalProfile::default_from_name(&e.value()) {
                    rebuilt(peak, picked, longitudinal);
                }
            },
        }
        if let TransversalProfile::SuperGaussian(shape) = transversal {
            SuperGaussianFields {
                id_prefix: id_prefix.clone(),
                shape,
                on_change: move |shaped| rebuilt(peak, TransversalProfile::SuperGaussian(shaped), longitudinal),
            }
        }
        LabeledSelect {
            id: format!("{id_prefix}-pump-longitudinal"),
            label: "Longitudinal profile".to_string(),
            options: select_options_from_enum_iterator(&longitudinal, None),
            onchange: move |e: Event<FormData>| {
                if let Some(picked) = LongitudinalProfile::default_from_name(&e.value()) {
                    rebuilt(peak, transversal, picked);
                }
            },
        }
        if let LongitudinalProfile::BeerLambert(profile) = longitudinal {
            BeerLambertFields {
                id_prefix,
                profile,
                on_change: move |absorbed| rebuilt(
                    peak,
                    transversal,
                    LongitudinalProfile::BeerLambert(absorbed),
                ),
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
    let millimeters = |axis: Length| axis.get::<millimeter>();
    rsx! {
        div { class: "amp-pump-nested",
            NumberField {
                id: format!("{id_prefix}-spot-sigma-x"),
                label: "Width σx (mm)".to_string(),
                value: millimeters(sigma.x),
                step: 0.5,
                min: Some(0.0),
                on_save: move |value: f64| {
                    rebuilt(
                        center,
                        millimeter!(value, millimeters(sigma.y)),
                        power,
                        theta,
                        rectangular,
                    );
                },
            }
            NumberField {
                id: format!("{id_prefix}-spot-sigma-y"),
                label: "Width σy (mm)".to_string(),
                value: millimeters(sigma.y),
                step: 0.5,
                min: Some(0.0),
                on_save: move |value: f64| {
                    rebuilt(
                        center,
                        millimeter!(millimeters(sigma.x), value),
                        power,
                        theta,
                        rectangular,
                    );
                },
            }
            NumberField {
                id: format!("{id_prefix}-spot-mu-x"),
                label: "Center x (mm)".to_string(),
                value: millimeters(center.x),
                step: 0.5,
                min: None,
                on_save: move |value: f64| {
                    rebuilt(
                        millimeter!(value, millimeters(center.y)),
                        sigma,
                        power,
                        theta,
                        rectangular,
                    );
                },
            }
            NumberField {
                id: format!("{id_prefix}-spot-mu-y"),
                label: "Center y (mm)".to_string(),
                value: millimeters(center.y),
                step: 0.5,
                min: None,
                on_save: move |value: f64| {
                    rebuilt(
                        millimeter!(millimeters(center.x), value),
                        sigma,
                        power,
                        theta,
                        rectangular,
                    );
                },
            }
            NumberField {
                id: format!("{id_prefix}-spot-power"),
                label: "Order (1 = Gaussian)".to_string(),
                value: power,
                step: 1.0,
                min: Some(0.0),
                on_save: move |value: f64| {
                    rebuilt(center, sigma, value, theta, rectangular);
                },
            }
            NumberField {
                id: format!("{id_prefix}-spot-theta"),
                label: "Rotation (°)".to_string(),
                value: theta.get::<degree>(),
                step: 5.0,
                min: None,
                on_save: move |value: f64| {
                    rebuilt(center, sigma, power, degree!(value), rectangular);
                },
            }
            div { class: "form-check",
                input {
                    class: "form-check-input",
                    r#type: "checkbox",
                    id: format!("{id_prefix}-spot-rectangular"),
                    checked: rectangular,
                    onchange: move |e| rebuilt(center, sigma, power, theta, e.checked()),
                }
                label {
                    class: "form-check-label text-secondary small",
                    r#for: format!("{id_prefix}-spot-rectangular"),
                    "Rectangular flanks"
                }
            }
        }
    }
}

/// The parameters of a [`BeerLambertProfile`]: how strongly the pump is absorbed, and which face it
/// enters through.
///
/// # Props
///
/// * `id_prefix` - distinguishes this editor's inputs from those of the other rows in the same list.
/// * `profile` - the absorption as the document currently holds it.
/// * `on_change` - handed the complete new profile whenever the user changes anything.
#[component]
fn BeerLambertFields(
    id_prefix: String,
    profile: BeerLambertProfile,
    on_change: EventHandler<BeerLambertProfile>,
) -> Element {
    let (absorption, direction) = (profile.absorption(), profile.direction());
    rsx! {
        div { class: "amp-pump-nested",
            NumberField {
                id: format!("{id_prefix}-beer-alpha"),
                label: "Absorption α (1/cm)".to_string(),
                value: absorption.get::<reciprocal_centimeter>(),
                step: 0.1,
                min: Some(0.0),
                on_save: move |value: f64| {
                    save(
                        BeerLambertProfile::new(reciprocal_centimeter!(value), direction),
                        on_change,
                    );
                },
            }
            LabeledSelect {
                id: format!("{id_prefix}-beer-direction"),
                label: "Pumped from".to_string(),
                options: select_options_from_enum_iterator(&direction, None),
                onchange: move |e: Event<FormData>| {
                    if let Some(picked) = PumpDirection::default_from_name(&e.value()) {
                        save(BeerLambertProfile::new(absorption, picked), on_change);
                    }
                },
            }
        }
    }
}

/// A number the user edits, saved when the field is left or `Enter` is pressed.
///
/// The value is only pushed upward once the user is done with it, not per keystroke — every save
/// here is a round trip to the document. Text that is not a number at all is refused on the spot and
/// the field falls back to what the document holds; a number the *core* refuses (a negative width,
/// say) is reported in the log and simply not applied, so the field keeps showing what was typed
/// until it is corrected.
///
/// # Props
///
/// * `id` - the input's own id, which its label points at.
/// * `label` - what the field is called.
/// * `value` - the number as the document currently holds it.
/// * `step` - how much the spinner arrows change it by.
/// * `min` - the smallest accepted value, if the quantity has one.
/// * `on_save` - handed the parsed number when the user is done editing.
#[component]
fn NumberField(
    id: String,
    label: String,
    value: f64,
    step: f64,
    min: Option<f64>,
    on_save: EventHandler<f64>,
) -> Element {
    // Same "compare and pull in" shape the gain factor next to this uses: the text is local while it
    // is being typed, and re-synced whenever the document's own value actually changes - a save
    // completing, an undo, an edit made somewhere else.
    let mut text = use_signal(|| format!("{value}"));
    let mut last_value = use_signal(|| value);
    if (*last_value.peek() - value).abs() > f64::EPSILON
        || last_value.peek().is_nan() != value.is_nan()
    {
        last_value.set(value);
        text.set(format!("{value}"));
    }

    let mut commit = move || {
        let raw = text.peek().trim().to_string();
        if let Ok(parsed) = raw.parse::<f64>() {
            on_save.call(parsed);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("'{raw}' is not a number"));
            text.set(format!("{value}"));
        }
    };

    rsx! {
        div { class: "form-floating border-start",
            input {
                class: "form-control bg-dark text-light",
                r#type: "number",
                id: id.as_str(),
                step: "{step}",
                min: min.map(|m| format!("{m}")),
                value: "{text}",
                oninput: move |e| text.set(e.value()),
                onblur: move |_| commit(),
                onkeydown: move |e| {
                    if e.key() == Key::Enter {
                        commit();
                    }
                },
            }
            label { class: "text-secondary", r#for: id, "{label}" }
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
