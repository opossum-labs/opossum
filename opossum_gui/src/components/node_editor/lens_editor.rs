use dioxus::prelude::*;
use opossum_backend::{millimeter, Proptype, RefrIndexConst, RefractiveIndexType};
use uom::si::{f64::Length, length::millimeter};

use crate::components::node_editor::node_editor_component::NodeChange;

#[component]
pub fn LensEditor(
    hide: bool,
    node_change: Signal<Option<NodeChange>>,
    front_radius: Length,
    rear_radius: Length,
    center_thickness: Length,
    refractive_index: f64

) -> Element {

    rsx! {
        div { class: "accordion-item bg-dark text-light", hidden: hide,
            h2 { class: "accordion-header", id: "lensHeading",
                button {
                    class: "accordion-button collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": "#lensCollapse",
                    "aria-expanded": "false",
                    "aria-controls": "lensCollapse",
                    "Lens Properties"
                }
            }
            div {
                id: "lensCollapse",
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": "lensHeading",
                "data-mdb-parent": "#accordionNodeConfig",
                div { class: "accordion-body  bg-dark",
                    div { class: "form-floating border-start", "data-mdb-input-init": "",
                        input {
                            class: "form-control bg-dark text-light form-control-sm",
                            r#type: "number",
                            id: "lens_front_radius",
                            name: "lens_front_radius",
                            placeholder: "Front radius in mm",
                            value: format!("{}", front_radius.get::<millimeter>()),
                            "readonly": false,
                            onchange: {
                                move |e: Event<FormData>| {
                                    if let Ok(front_radius) = e.data.parsed::<f64>() {
                                        node_change.set(Some(NodeChange::Property(
                                            "front curvature".to_owned(),
                                            serde_json::to_value(Proptype::Length(millimeter!(front_radius)))
                                            .unwrap(),
                                        )))
                                    }
                                }
                            },
                        }
                        label { class: "form-label text-secondary", r#for: "lens_front_radius", "Front radius in mm" }
                    }

                    div { class: "form-floating border-start", "data-mdb-input-init": "",
                        input {
                            class: "form-control bg-dark text-light form-control-sm",
                            r#type: "number",
                            id: "lens_rear_radius",
                            name: "lens_rear_radius",
                            placeholder: "Rear radius in mm",
                            value: format!("{}", rear_radius.get::<millimeter>()),
                            "readonly": false,
                            onchange: {
                                move |e: Event<FormData>| {
                                    if let Ok(rear_radius) = e.data.parsed::<f64>() {
                                        node_change.set(Some(NodeChange::Property(
                                            "rear curvature".to_owned(),
                                            serde_json::to_value(Proptype::Length(millimeter!(rear_radius)))
                                            .unwrap(),
                                        )))
                                    }
                                }
                            },
                        }
                        label { class: "form-label text-secondary", r#for: "lens_rear_radius", "Rear radius in mm" }
                    }

                    div { class: "form-floating border-start", "data-mdb-input-init": "",
                        input {
                            class: "form-control bg-dark text-light form-control-sm",
                            r#type: "number",
                            min: "0.0000000001",
                            id: "lens_center_thickness",
                            name: "lens_center_thickness",
                            placeholder: "Center thickness in mm",
                            value: format!("{}", center_thickness.get::<millimeter>()),
                            "readonly": false,
                            onchange: {
                                move |e: Event<FormData>| {
                                    if let Ok(center_thickness) = e.data.parsed::<f64>() {
                                        node_change.set(Some(NodeChange::Property(
                                            "center thickness".to_owned(),
                                            serde_json::to_value(Proptype::Length(millimeter!(center_thickness)))
                                            .unwrap(),
                                        )))
                                    }
                                }
                            },
                        }
                        label { class: "form-label text-secondary", r#for: "lens_center_thickness", "Center thickness in mm" }
                    }

                    div { class: "form-floating border-start", "data-mdb-input-init": "",
                        input {
                            class: "form-control bg-dark text-light form-control-sm",
                            r#type: "number",
                            min: "1.",
                            id: "lens_refr_index",
                            name: "lens_refr_index",
                            placeholder: "Refractive Index",
                            value: format!("{refractive_index}"),
                            "readonly": false,
                            onchange: {
                                move |e: Event<FormData>| {
                                    if let Ok(ref_ind) = e.data.parsed::<f64>() {
                                        if ref_ind > 1. && ref_ind.is_finite(){
                                            node_change.set(Some(NodeChange::Property(
                                                "refractive index".to_owned(),
                                                serde_json::to_value(Proptype::RefractiveIndex(RefractiveIndexType::Const(RefrIndexConst::new(ref_ind).unwrap())))
                                                .unwrap(),
                                            )))
                                        }
                                    }
                                }
                            },
                        }
                        label { class: "form-label text-secondary", r#for: "lens_refr_index", "Refractive Index" }
                    }

                    //todo refractive index
                }
            }
        }
    }
    // }
}