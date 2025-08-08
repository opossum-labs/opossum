use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput,
    optical_node_editor::properties_editor::light_data_editor::ray_source_editor::distribution_editor::DistributionEditor,
};
use dioxus::prelude::*;
use opossum_backend::{
    millimeter,
    ray_data_builder::{PointSrc, RayDataBuilder},
};
use uom::si::length::millimeter;

#[component]
pub fn ReferenceLengthEditor(
    ray_data_builder_sig: Signal<RayDataBuilder>,
    point_src: PointSrc,
) -> Element {
    rsx! {
        LabeledInput {
            id: "pointsrcRefLength",
            label: "Reference Length in mm",
            value: format!("{}", point_src.reference_length().get::<millimeter>()),
            onchange: CallbackWrapper::new({
                let point_src = point_src;
                move |e: Event<FormData>| {
                    let mut point_src = point_src.clone();
                    if let Ok(ref_length) = e.data.parsed::<f64>() {
                        point_src.set_reference_length(millimeter!(ref_length));
                        ray_data_builder_sig.set(RayDataBuilder::PointSrc(point_src));
                    }
                }
            }),
            r#type: "number",
        }
    }
}

#[component]
pub fn PointSourceEditor(ray_data_builder_sig: Signal<RayDataBuilder>) -> Element {
    match &*ray_data_builder_sig.read() {
        RayDataBuilder::PointSrc(point_src) => {
            rsx! {
                ReferenceLengthEditor { ray_data_builder_sig, point_src: point_src.clone() }
                DistributionEditor { ray_data_builder_sig }
            }
        }
        _ => {
            rsx! {}
        }
    }
}
