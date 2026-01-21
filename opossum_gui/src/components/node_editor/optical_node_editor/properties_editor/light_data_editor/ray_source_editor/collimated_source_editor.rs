use crate::components::{
    logger::LogResultExt, node_editor::inputs::input_components::LabeledInput,
};
use dioxus::prelude::*;
use opossum_core::millimeter;
use opossum_core::prelude::{PointSrc, RayDataBuilder};
use uom::si::length::millimeter;

#[component]
pub fn ReferenceLengthEditor(point_src: PointSrc) -> Element {
    let mut ray_data_builder_sig = use_context::<Signal<RayDataBuilder>>();
    rsx! {
        LabeledInput {
            id: "pointsrcRefLength",
            label: "Reference Length in mm",
            value: format!("{}", point_src.reference_length().get::<millimeter>()),
            onchange: move |e: Event<FormData>| {
                let mut point_src = point_src.clone();
                if let Ok(ref_length) = e.data.value().parse::<f64>() {
                    point_src
                        .set_reference_length(millimeter!(ref_length))
                        .log_err_with_context(
                            "validation failed in `set_reference_length` of PointSrc",
                        );
                    ray_data_builder_sig.set(RayDataBuilder::PointSrc(point_src));
                }
            },
            r#type: "number",
        }
    }
}
