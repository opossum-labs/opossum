use crate::components::{
    logger::LogResultExt, node_editor::inputs::input_components::NodeConfigUnitInput,
};
use dioxus::prelude::*;
use opossum_core::{meter, prelude::PointSrc};

#[component]
pub fn ReferenceLengthEditor(
    point_src: PointSrc,
    ray_data_handler: EventHandler<PointSrc>,
    readonly: bool,
) -> Element {
    rsx! {
        NodeConfigUnitInput {
            id: "pointsrcRefLength",
            label: "Reference Length",
            value: point_src.reference_length().value,
            base_unit: "m",
            readonly,
            onchange: move |new_length: f64| {
                let mut point_src = point_src.clone();

                point_src
                    .set_reference_length(meter!(new_length))
                    .log_err_with_context(
                        "validation failed in `set_reference_length` of PointSrc",
                    );
                ray_data_handler.call(point_src);
            },
        }
    }
}
