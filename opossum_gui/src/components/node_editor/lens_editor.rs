use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledInput},
    node_editor_component::NodeChange,
};
use dioxus::prelude::*;
use opossum_backend::{millimeter, NodeAttr, Proptype, RefrIndexConst, RefractiveIndexType};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn LensFrontCurvatureInput(front_curvature: Length) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputLensFrontCurvature",
            label: "Front curvature in mm",
            value: format!("{}", front_curvature.get::<millimeter>()),
            r#type: "number",
            onchange: Some(lens_geometry_onchange(node_change_signal, "front curvature")),
        }
    }
}
#[component]
pub fn LensRearCurvatureInput(rear_curvature: Length) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputLensRearCurvature",
            label: "Rear curvature in mm",
            value: format!("{}", rear_curvature.get::<millimeter>()),
            r#type: "number",
            onchange: Some(lens_geometry_onchange(node_change_signal, "rear curvature")),
        }
    }
}

#[component]
pub fn LensCenterThicknessInput(center_thickness: Length) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputCenterThickness",
            label: "Center thickness in mm",
            value: format!("{}", center_thickness.get::<millimeter>()),
            r#type: "number",
            min: Some("0.0000000001"),
            onchange: Some(lens_geometry_onchange(node_change_signal, "center thickness")),
        }
    }
}

#[component]
pub fn LensRefractiveIndexInput(refractive_index: f64) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    rsx! {
        LabeledInput {
            id: "inputLensRefractiveIndex",
            label: "Refractive index",
            value: format!("{refractive_index}"),
            r#type: "number",
            min: Some("1.0"),
            onchange: Some(lens_refractive_index_onchange(node_change_signal)),
        }
    }
}

fn lens_refractive_index_onchange(
    mut signal: Signal<Option<NodeChange>>,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        if let Ok(ref_ind) = e.data.value().parse::<f64>() {
            if ref_ind > 1. && ref_ind.is_finite() {
                signal.set(Some(NodeChange::Property(
                    "refractive index".to_owned(),
                    serde_json::to_value(Proptype::RefractiveIndex(RefractiveIndexType::Const(
                        RefrIndexConst::new(ref_ind).unwrap(),
                    )))
                    .unwrap(),
                )));
            }
        }
    })
}

fn lens_geometry_onchange(
    mut signal: Signal<Option<NodeChange>>,
    property_string: &'static str,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(Some(NodeChange::Property(
                property_string.to_owned(),
                serde_json::to_value(Proptype::Length(millimeter!(length))).unwrap(),
            )));
        }
    })
}

#[component]
pub fn LensEditor(
    hidden: bool,
    node_change: Signal<Option<NodeChange>>,
    lens_properties: LensProperties,
) -> Element {
    let accordion_content = vec![rsx! {
            LensFrontCurvatureInput {front_curvature: lens_properties.front_curvature()}
            LensRearCurvatureInput {rear_curvature: lens_properties.rear_curvature()}
            LensCenterThicknessInput{center_thickness: lens_properties.center_thickness()}
            //todo: refractiveindex for sellmeier, schott, etc.
            LensRefractiveIndexInput {refractive_index: lens_properties.refractive_index()},
    }];

    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Lens Properties",
            header_id: "lensHeading",
            parent_id: "accordionNodeConfig",
            content_id: "lensCollapse",
            hidden,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub struct LensProperties {
    front_curvature: Length,
    rear_curvature: Length,
    center_thickness: Length,
    refractive_index: f64,
}

impl LensProperties {
    fn new(
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        refractive_index: f64,
    ) -> Self {
        Self {
            front_curvature,
            rear_curvature,
            center_thickness,
            refractive_index,
        }
    }
    fn front_curvature(&self) -> Length {
        self.front_curvature
    }
    fn rear_curvature(&self) -> Length {
        self.rear_curvature
    }
    fn center_thickness(&self) -> Length {
        self.center_thickness
    }
    fn refractive_index(&self) -> f64 {
        self.refractive_index
    }
}

impl Default for LensProperties {
    fn default() -> Self {
        Self::new(millimeter!(500.), millimeter!(-500.), millimeter!(10.), 1.5)
    }
}

impl From<&NodeAttr> for LensProperties {
    fn from(node_attr: &NodeAttr) -> Self {
        let front_curvature = if let Some(Proptype::Length(front_curvature)) =
            node_attr.get_property("front curvature").ok()
        {
            front_curvature.clone()
        } else {
            millimeter!(500.)
        };
        let rear_curvature = if let Some(Proptype::Length(rear_curvature)) =
            node_attr.get_property("rear curvature").ok()
        {
            rear_curvature.clone()
        } else {
            millimeter!(-500.)
        };
        let center_thickness = if let Some(Proptype::Length(center_thickness)) =
            node_attr.get_property("center thickness").ok()
        {
            center_thickness.clone()
        } else {
            millimeter!(10.)
        };
        let refractive_index =
            if let Some(Proptype::RefractiveIndex(RefractiveIndexType::Const(ri))) =
                node_attr.get_property("refractive index").ok()
            {
                ri.refractive_index()
            } else {
                1.5
            };
        Self::new(
            front_curvature,
            rear_curvature,
            center_thickness,
            refractive_index,
        )
    }
}
