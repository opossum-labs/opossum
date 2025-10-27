use crate::components::node_editor::{CallbackWrapper, inputs::input_components::LabeledInput};
use dioxus::prelude::*;
use opossum_core::{
    millimeter,
    prelude::{PointSrc, RayDataBuilder},
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
            value: format!("{:.3}", point_src.reference_length().get::<millimeter>()),
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
