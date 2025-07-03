use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{
    nanometer, refr_index_schott::RefrIndexSchott, Proptype, RefrIndexConrady, RefrIndexSellmeier1,
    RefractiveIndexType,
};
use strum::IntoEnumIterator;
use uom::si::length::nanometer;

use crate::components::node_editor::{
    inputs::{
        input_components::{LabeledSelect, RowedInputs},
        InputData, InputParam,
    },
    CallbackWrapper,
};

#[component]
pub fn RefractiveIndexEditor(property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    if let Proptype::RefractiveIndex(ref_ind_type) = &*prop_type_sig.read() {
        let select_id = format!("lengthProperty{property_key}").to_camel_case();
        rsx! {
            LabeledSelect {
                id: select_id,
                label: "Refractive index definition",
                options: get_refractive_index_options(ref_ind_type),
                onchange: move |e: Event<FormData>| {
                    let val = e.value();
                    if let Some(ref_ind_type) = RefractiveIndexType::default_from_name(val.as_str()) {
                        prop_type_sig.set(ref_ind_type.into());
                    }
                },
            }
            div { class: "accordion-content-wrapper-div border-start",
                RowedInputs { inputs: get_refractive_index_input_data(ref_ind_type, prop_type_sig) }
            }
        }
    } else {
        rsx! {}
    }
}

fn get_refractive_index_options(ref_ind_type: &RefractiveIndexType) -> Vec<(bool, String)> {
    let mut ref_ind_options = Vec::<(bool, String)>::new();

    for ri_type in RefractiveIndexType::iter() {
        if std::mem::discriminant(&ri_type) == std::mem::discriminant(ref_ind_type) {
            ref_ind_options.push((true, format!("{ri_type}")));
        } else {
            ref_ind_options.push((false, format!("{ri_type}")));
        }
    }
    ref_ind_options
}

fn get_ref_ind_conrady_input_data(
    ref_ind_type: &RefractiveIndexType,
    ref_ind: &RefrIndexConrady,
    prop_type_sig: Signal<Proptype>,
) -> Vec<InputData> {
    vec![
        InputData::new(
            InputParam::WaveLengthStart,
            &"refractiveIndexConradywvl1Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthStart),
            format!("{}", ref_ind.wavelength_range().start.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::WaveLengthEnd,
            &"refractiveIndexConradywvl2Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthEnd),
            format!("{}", ref_ind.wavelength_range().end.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::Conrady0,
            &"refractiveIndexConradyAInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Conrady0),
            format!("{}", ref_ind.n0()),
        ),
        InputData::new(
            InputParam::Conrady1,
            &"refractiveIndexConradyBInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Conrady1),
            format!("{}", ref_ind.a()),
        ),
        InputData::new(
            InputParam::Conrady2,
            &"refractiveIndexConradyICnput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Conrady2),
            format!("{}", ref_ind.b()),
        ),
    ]
}
fn get_ref_ind_schott_input_data(
    ref_ind_type: &RefractiveIndexType,
    ref_ind: &RefrIndexSchott,
    prop_type_sig: Signal<Proptype>,
) -> Vec<InputData> {
    vec![
        InputData::new(
            InputParam::WaveLengthStart,
            &"refractiveIndexSchottwvl1Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthStart),
            format!("{}", ref_ind.wavelength_range().start.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::WaveLengthEnd,
            &"refractiveIndexSchottwvl2Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthEnd),
            format!("{}", ref_ind.wavelength_range().end.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::Schott0,
            &"refractiveIndexSchottAInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott0),
            format!("{}", ref_ind.a0()),
        ),
        InputData::new(
            InputParam::Schott1,
            &"refractiveIndexSchottBInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott1),
            format!("{}", ref_ind.a1()),
        ),
        InputData::new(
            InputParam::Schott2,
            &"refractiveIndexSchottCInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott2),
            format!("{}", ref_ind.a2()),
        ),
        InputData::new(
            InputParam::Schott3,
            &"refractiveIndexSchottDInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott3),
            format!("{}", ref_ind.a3()),
        ),
        InputData::new(
            InputParam::Schott4,
            &"refractiveIndexSchottEInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott4),
            format!("{}", ref_ind.a4()),
        ),
        InputData::new(
            InputParam::Schott5,
            &"refractiveIndexSchottFInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Schott5),
            format!("{}", ref_ind.a5()),
        ),
    ]
}
fn get_ref_ind_sellmeier_input_data(
    ref_ind_type: &RefractiveIndexType,
    ref_ind: &RefrIndexSellmeier1,
    prop_type_sig: Signal<Proptype>,
) -> Vec<InputData> {
    vec![
        InputData::new(
            InputParam::WaveLengthStart,
            &"refractiveIndexSellmeierwvl1Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthStart),
            format!("{}", ref_ind.wavelength_range().start.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::WaveLengthEnd,
            &"refractiveIndexSellmeierwvl2Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::WaveLengthEnd),
            format!("{}", ref_ind.wavelength_range().end.get::<nanometer>()),
        ),
        InputData::new(
            InputParam::Sellmeierk1,
            &"refractiveIndexSellmeierk1Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierk1),
            format!("{}", ref_ind.k1()),
        ),
        InputData::new(
            InputParam::Sellmeierl1,
            &"refractiveIndexSellmeierl1Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierl1),
            format!("{}", ref_ind.l1()),
        ),
        InputData::new(
            InputParam::Sellmeierk2,
            &"refractiveIndexSellmeierk2Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierk2),
            format!("{}", ref_ind.k2()),
        ),
        InputData::new(
            InputParam::Sellmeierl2,
            &"refractiveIndexSellmeierl2Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierl2),
            format!("{}", ref_ind.l2()),
        ),
        InputData::new(
            InputParam::Sellmeierk3,
            &"refractiveIndexSellmeierk3Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierk3),
            format!("{}", ref_ind.k3()),
        ),
        InputData::new(
            InputParam::Sellmeierl3,
            &"refractiveIndexSellmeierl3Input".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::Sellmeierl3),
            format!("{}", ref_ind.l3()),
        ),
    ]
}
fn get_refractive_index_input_data(
    ref_ind_type: &RefractiveIndexType,
    prop_type_sig: Signal<Proptype>,
) -> Vec<InputData> {
    match ref_ind_type {
        RefractiveIndexType::Const(ref_ind) => vec![InputData::new(
            InputParam::RefractiveIndex,
            &"refractiveIndexConstInput".to_string(),
            on_refractive_index_change(ref_ind_type, prop_type_sig, InputParam::RefractiveIndex),
            format!("{}", ref_ind.refractive_index()),
        )],
        RefractiveIndexType::Sellmeier1(ref_ind) => {
            get_ref_ind_sellmeier_input_data(ref_ind_type, ref_ind, prop_type_sig)
        }
        RefractiveIndexType::Schott(ref_ind) => {
            get_ref_ind_schott_input_data(ref_ind_type, ref_ind, prop_type_sig)
        }
        RefractiveIndexType::Conrady(ref_ind) => {
            get_ref_ind_conrady_input_data(ref_ind_type, ref_ind, prop_type_sig)
        }
    }
}

fn on_refractive_index_change(
    ref_ind_type: &RefractiveIndexType,
    mut prop_type_sig: Signal<Proptype>,
    input_param: InputParam,
) -> CallbackWrapper {
    CallbackWrapper::new({
        let ref_ind_type: RefractiveIndexType = ref_ind_type.clone();
        move |e: Event<FormData>| {
            if let Ok(value) = e.value().parse::<f64>() {
                match ref_ind_type.clone() {
                    RefractiveIndexType::Const(mut ref_ind) => {
                        ref_ind.set_refractive_index(value);
                        prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Const(
                            ref_ind,
                        )));
                    }
                    RefractiveIndexType::Sellmeier1(ref_ind) => {
                        set_ref_ind_sellmeier_callback(input_param, value, ref_ind, prop_type_sig);
                    }
                    RefractiveIndexType::Schott(ref_ind) => {
                        set_ref_ind_schott_callback(input_param, value, ref_ind, prop_type_sig);
                    }
                    RefractiveIndexType::Conrady(ref_ind) => {
                        set_ref_ind_conrady_callback(input_param, value, ref_ind, prop_type_sig);
                    }
                }
            }
        }
    })
}

fn set_ref_ind_schott_callback(
    input_param: InputParam,
    value: f64,
    mut ref_ind: RefrIndexSchott,
    mut prop_type_sig: Signal<Proptype>,
) {
    match input_param {
        InputParam::WaveLengthStart => {
            ref_ind.set_wavelength_range_start(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::WaveLengthEnd => {
            ref_ind.set_wavelength_range_end(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott0 => {
            ref_ind.set_a0(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott1 => {
            ref_ind.set_a1(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott2 => {
            ref_ind.set_a2(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott3 => {
            ref_ind.set_a3(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott4 => {
            ref_ind.set_a4(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        InputParam::Schott5 => {
            ref_ind.set_a5(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Schott(
                ref_ind,
            )));
        }
        _ => {}
    }
}

fn set_ref_ind_conrady_callback(
    input_param: InputParam,
    value: f64,
    mut ref_ind: RefrIndexConrady,
    mut prop_type_sig: Signal<Proptype>,
) {
    match input_param {
        InputParam::WaveLengthStart => {
            ref_ind.set_wavelength_range_start(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Conrady(
                ref_ind,
            )));
        }
        InputParam::WaveLengthEnd => {
            ref_ind.set_wavelength_range_end(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Conrady(
                ref_ind,
            )));
        }
        InputParam::Conrady0 => {
            ref_ind.set_n0(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Conrady(
                ref_ind,
            )));
        }
        InputParam::Conrady1 => {
            ref_ind.set_a(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Conrady(
                ref_ind,
            )));
        }
        InputParam::Conrady2 => {
            ref_ind.set_b(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Conrady(
                ref_ind,
            )));
        }
        _ => {}
    }
}
fn set_ref_ind_sellmeier_callback(
    input_param: InputParam,
    value: f64,
    mut ref_ind: RefrIndexSellmeier1,
    mut prop_type_sig: Signal<Proptype>,
) {
    match input_param {
        InputParam::WaveLengthStart => {
            ref_ind.set_wavelength_range_start(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::WaveLengthEnd => {
            ref_ind.set_wavelength_range_end(nanometer!(value));
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierk1 => {
            ref_ind.set_k1(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierk2 => {
            ref_ind.set_k2(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierk3 => {
            ref_ind.set_k3(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierl1 => {
            ref_ind.set_l1(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierl2 => {
            ref_ind.set_l2(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        InputParam::Sellmeierl3 => {
            ref_ind.set_l3(value);
            prop_type_sig.set(Proptype::RefractiveIndex(RefractiveIndexType::Sellmeier1(
                ref_ind,
            )));
        }
        _ => {}
    }
}
