#![allow(clippy::derive_partial_eq_without_eq)]
use std::path::Path;

use dioxus::prelude::*;
use opossum_backend::{
    degree, joule,
    light_data_builder::LightDataBuilder,
    micrometer, millimeter, nanometer,
    ray_data_builder::{ImageSrc, RayDataBuilder},
    DefaultFromName, Rays,
};
use uom::si::{
    angle::degree,
    energy::joule,
    length::{micrometer, millimeter, nanometer},
};

use crate::components::node_editor::{
    inputs::{
        input_components::{LabeledInput, LabeledSelect, RowedInputs},
        select_options_from_enum_iterator, InputData, InputParam, IntoInputData,
        IntoInputDataStrings,
    },
    CallbackWrapper,
};
use strum::{EnumIter, IntoEnumIterator};

#[derive(Clone, Copy, EnumIter, Eq, PartialEq)]
enum ImageSrcParam {
    FPath,
    PxlSize,
    Energy,
    Wavelength,
    ConeAngle,
}

impl From<ImageSrcParam> for InputParam {
    fn from(value: ImageSrcParam) -> Self {
        match value {
            ImageSrcParam::FPath => Self::FilePath("File:"),
            ImageSrcParam::PxlSize => Self::F64("Pixel size in µm"),
            ImageSrcParam::Energy => Self::Energy("Energy in J"),
            ImageSrcParam::Wavelength => Self::Length("Wavelength in nm"),
            ImageSrcParam::ConeAngle => Self::Angle("Cone Angle in degrees"),
        }
    }
}

impl IntoInputDataStrings<ImageSrc> for ImageSrcParam {
    fn create_id_string(&self) -> String {
        let id_str = match self {
            Self::FPath => "File:",
            Self::PxlSize => "PxlSize",
            Self::Energy => "Energy",
            Self::Wavelength => "Wavelength",
            Self::ConeAngle => "ConeAngle",
        };

        format!("rayTypeImageSrc{id_str}Input")
    }
    fn create_value_string(&self, obj: &ImageSrc) -> String {
        match self {
            Self::FPath => obj
                .file_path()
                .file_name()
                .map_or("no file selected", |f| {
                    f.to_str().unwrap_or("no file selected")
                })
                .to_string(),
            Self::PxlSize => format!("{:.3e}", obj.pixel_size().get::<micrometer>()),
            Self::Energy => format!("{:.3e}", obj.energy().get::<joule>()),
            Self::Wavelength => format!("{:.3e}", obj.wavelength().get::<nanometer>()),
            Self::ConeAngle => format!("{:.3e}", obj.cone_angle().get::<degree>()),
        }
    }
}

impl IntoInputData<f64, ImageSrc, LightDataBuilder> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<f64> {
        let e_value = e.value();
        e_value.parse::<f64>().ok()
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut ImageSrc, f64) {
        match self {
            Self::PxlSize => {
                move |obj: &mut ImageSrc, val: f64| obj.set_pixel_size(micrometer!(val))
            }
            Self::Energy => move |obj: &mut ImageSrc, val: f64| obj.set_energy(joule!(val)),
            Self::Wavelength => {
                move |obj: &mut ImageSrc, val: f64| obj.set_wavelength(nanometer!(val))
            }
            Self::ConeAngle => move |obj: &mut ImageSrc, val: f64| obj.set_cone_angle(degree!(val)),
            Self::FPath => move |_: &mut ImageSrc, _: f64| {},
        }
    }
}

impl IntoInputData<String, ImageSrc, LightDataBuilder> for ImageSrcParam {
    fn parse_value(&self, e: Event<FormData>) -> Option<String> {
        e.files().and_then(|file_engine| {
            let files = file_engine.files();
            if files.is_empty() {
                None
            } else {
                Some(files[0].clone())
            }
        })
    }
    fn setter_from_obj(&self) -> impl FnMut(&mut ImageSrc, String) {
        if *self == Self::FPath {
            move |obj: &mut ImageSrc, val: String| obj.set_file_path(Path::new(&val).to_path_buf())
        } else {
            move |_: &mut ImageSrc, _: String| {}
        }
    }
}

fn get_image_source_input_params(
    img_src: &ImageSrc,
    light_data_builder_sig: Signal<LightDataBuilder>,
) -> Vec<InputData> {
    let mut input_data = Vec::<InputData>::new();
    for enum_variant in ImageSrcParam::iter() {
        match enum_variant {
            ImageSrcParam::FPath => {
                input_data.push(
                    IntoInputData::<String, ImageSrc, LightDataBuilder>::to_input_data(
                        &enum_variant,
                        img_src.clone(),
                        light_data_builder_sig,
                    ),
                );
            }
            _ => input_data.push(
                IntoInputData::<f64, ImageSrc, LightDataBuilder>::to_input_data(
                    &enum_variant,
                    img_src.clone(),
                    light_data_builder_sig,
                ),
            ),
        }
    }
    input_data
}

#[component]
pub fn ImageSourceEditor(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    if let LightDataBuilder::Geometric(RayDataBuilder::Image(img_src)) =
        &*light_data_builder_sig.read()
    {
        let inputs = get_image_source_input_params(img_src, light_data_builder_sig);
        rsx! {
            RowedInputs { inputs }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn ReferenceLengthEditor(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    if let LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src)) =
        &*light_data_builder_sig.read()
    {
        rsx! {
            LabeledInput {
                id: "pointsrcRefLength",
                label: "Reference Length in mm",
                value: format!("{}", point_src.reference_length().get::<millimeter>()),
                onchange: CallbackWrapper::new({
                    let point_src = point_src.clone();
                    move |e: Event<FormData>| {
                        let mut point_src = point_src.clone();
                        if let Ok(ref_length) = e.data.parsed::<f64>() {
                            point_src.set_reference_length(millimeter!(ref_length));
                            light_data_builder_sig
                                .set(
                                    LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src)),
                                );
                        }
                    }
                }),
                r#type: "number",
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn RayDataBuilderSelector(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    if let LightDataBuilder::Geometric(rdb) = &*light_data_builder_sig.read() {
        rsx! {
            LabeledSelect {
                id: "selectRaySourceType",
                label: "Rays Type",
                options: select_options_from_enum_iterator(
                    rdb,
                    Some(&[&RayDataBuilder::Raw(Rays::default())]),
                ),
                onchange: move |e: Event<FormData>| {
                    let val = e.value();
                    if let Some(rdb) = RayDataBuilder::default_from_name(val.as_str()) {
                        light_data_builder_sig.set(LightDataBuilder::Geometric(rdb));
                    }
                },
            }
        }
    } else {
        rsx! {}
    }
}
