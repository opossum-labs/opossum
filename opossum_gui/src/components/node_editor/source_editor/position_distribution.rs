use std::fmt::Display;

use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledInput, LabeledSelect},
    source_editor::LightDataBuilderHistory,
};
use dioxus::prelude::*;
use itertools::Itertools;
use opossum_backend::{
    light_data_builder::LightDataBuilder, millimeter, ray_data_builder::RayDataBuilder, PosDistType,
};
use strum::IntoEnumIterator;
use uom::si::length::millimeter;

#[derive(Clone, PartialEq)]
pub enum PosDistParam {
    Rings,
    Radius,
    CenterX,
    CenterY,
    LengthX,
    LengthY,
    PointsX,
    PointsY,
}

impl PosDistParam {
    pub fn input_label(&self) -> String {
        match self {
            PosDistParam::Rings => "Number of Rings".to_string(),
            PosDistParam::Radius => "Radius in mm".to_string(),
            PosDistParam::CenterX => "Center X in mm".to_string(),
            PosDistParam::CenterY => "Center Y in mm".to_string(),
            PosDistParam::LengthX => "Length X in mm".to_string(),
            PosDistParam::LengthY => "Length Y in mm".to_string(),
            PosDistParam::PointsX => "#Points X".to_string(),
            PosDistParam::PointsY => "#Points Y".to_string(),
        }
    }

    pub fn min_value(&self) -> &'static str {
        match self {
            PosDistParam::Rings | PosDistParam::PointsX | PosDistParam::PointsY => "1",
            PosDistParam::Radius | PosDistParam::LengthX | PosDistParam::LengthY => "1e-9",
            PosDistParam::CenterX | PosDistParam::CenterY => "-1e9",
        }
    }
    pub fn step_value(&self) -> &'static str {
        "1"
    }
}

impl Display for PosDistParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param = match self {
            PosDistParam::Rings => "Rings",
            PosDistParam::Radius => "Radius",
            PosDistParam::CenterX => "CenterX",
            PosDistParam::CenterY => "CenterY",
            PosDistParam::LengthX => "LengthX",
            PosDistParam::LengthY => "lengthY",
            PosDistParam::PointsX => "PointsX",
            PosDistParam::PointsY => "PointsY",
        };
        write!(f, "{param}")
    }
}

fn get_input_params(pos_dist_type: PosDistType) -> Vec<(PosDistParam, String)> {
    match pos_dist_type {
        PosDistType::Random(random) => vec![
            (
                PosDistParam::LengthX,
                format!("{}", random.side_length_x().get::<millimeter>()),
            ),
            (
                PosDistParam::LengthY,
                format!("{}", random.side_length_y().get::<millimeter>()),
            ),
            (PosDistParam::PointsX, format!("{}", random.nr_of_points())),
        ],
        PosDistType::Grid(grid) => vec![
            (
                PosDistParam::LengthX,
                format!("{}", grid.side_length().0.get::<millimeter>()),
            ),
            (
                PosDistParam::LengthY,
                format!("{}", grid.side_length().1.get::<millimeter>()),
            ),
            (PosDistParam::PointsX, format!("{}", grid.nr_of_points().0)),
            (PosDistParam::PointsY, format!("{}", grid.nr_of_points().1)),
        ],
        PosDistType::HexagonalTiling(hexagonal_tiling) => vec![
            (
                PosDistParam::Rings,
                format!("{}", hexagonal_tiling.nr_of_hex_along_radius()),
            ),
            (
                PosDistParam::Radius,
                format!("{}", hexagonal_tiling.radius().get::<millimeter>()),
            ),
            (
                PosDistParam::CenterX,
                format!("{}", hexagonal_tiling.center().x.get::<millimeter>()),
            ),
            (
                PosDistParam::CenterY,
                format!("{}", hexagonal_tiling.center().y.get::<millimeter>()),
            ),
        ],
        PosDistType::Hexapolar(hexapolar) => vec![
            (PosDistParam::Rings, format!("{}", hexapolar.nr_of_rings())),
            (
                PosDistParam::Radius,
                format!("{}", hexapolar.radius().get::<millimeter>()),
            ),
        ],
        PosDistType::FibonacciRectangle(fibonacci_rectangle) => vec![
            (
                PosDistParam::LengthX,
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_x().get::<millimeter>()
                ),
            ),
            (
                PosDistParam::LengthY,
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_y().get::<millimeter>()
                ),
            ),
            (
                PosDistParam::PointsX,
                format!("{}", fibonacci_rectangle.nr_of_points()),
            ),
        ],
        PosDistType::FibonacciEllipse(fibonacci_ellipse) => vec![
            (
                PosDistParam::LengthX,
                format!("{}", fibonacci_ellipse.radius_x().get::<millimeter>()),
            ),
            (
                PosDistParam::LengthY,
                format!("{}", fibonacci_ellipse.radius_y().get::<millimeter>()),
            ),
            (
                PosDistParam::PointsX,
                format!("{}", fibonacci_ellipse.nr_of_points()),
            ),
        ],
        PosDistType::Sobol(sobol_dist) => vec![
            (
                PosDistParam::LengthX,
                format!("{}", sobol_dist.side_length_x().get::<millimeter>()),
            ),
            (
                PosDistParam::LengthY,
                format!("{}", sobol_dist.side_length_y().get::<millimeter>()),
            ),
            (
                PosDistParam::PointsX,
                format!("{}", sobol_dist.nr_of_points()),
            ),
        ],
    }
}

#[component]
pub fn RayPositionDistributionSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist =
        PosDistSelection::try_from(light_data_builder_sig.read().get_current().clone());

    if let Ok(rpd) = rays_pos_dist {
        rsx! {
            LabeledSelect {
                id: "selectRaysPosDistribution",
                label: "Rays Position Distribution",
                options: rpd.get_option_elements(),
                hidden: !show,
                onchange: move |e: Event<FormData>| {
                    light_data_builder_sig
                        .with_mut(|ldb| {
                            let value = e.value();
                            ldb.set_current_or_default(value.as_str());
                        });
                },
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn RayDistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist = light_data_builder_sig
        .read()
        .get_current_pos_dist_type()
        .clone();

    rsx! {
        div { hidden: !show,
            {
                if let Some(pos_dist_type) = rays_pos_dist {
                    rsx! {
                        NodePosDistInput { pos_dist_type, light_data_builder_sig }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[component]
pub fn NodePosDistInput(
    pos_dist_type: PosDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let pos_dist_params = get_input_params(pos_dist_type);
    rsx! {
        for chunk in pos_dist_params.iter().chunks(2) {
            {
                let inputs: Vec<&(PosDistParam, String)> = chunk
                    .collect::<Vec<&(PosDistParam, String)>>();
                if inputs.len() == 2 {
                    let (param1, value1) = &inputs[0];
                    let (param2, value2) = &inputs[1];
                    let id1 = format!("node{pos_dist_type}{param1}Input");
                    let id2 = format!("node{pos_dist_type}{param2}Input");
                    let label1 = param1.input_label();
                    let label2 = param2.input_label();
                    rsx! {
                        div { class: "row gy-1 gx-2",
                            div { class: "col-sm",
                                LabeledInput {
                                    id: id1,
                                    label: label1,
                                    value: value1.clone(),
                                    step: Some(param1.step_value()),
                                    min: Some(param1.min_value()),
                                    onchange: use_on_pos_dist_input_change(
                                        pos_dist_type.clone(),
                                        param1.clone(),
                                        light_data_builder_sig.clone(),
                                    ),
                                    r#type: "number",
                                }
                            }
                            div { class: "col-sm",
                                LabeledInput {
                                    id: id2,
                                    label: label2,
                                    value: value2.clone(),
                                    step: Some(param2.step_value()),
                                    min: Some(param2.min_value()),
                                    onchange: use_on_pos_dist_input_change(
                                        pos_dist_type.clone(),
                                        param2.clone(),
                                        light_data_builder_sig.clone(),
                                    ),
                                    r#type: "number",
                                }
                            }
                        }
                    }
                } else if inputs.len() == 1 {
                    let (param, value) = &inputs[0];
                    let id = format!("node{pos_dist_type}{param}Input");
                    let label = param.input_label();
                    rsx! {
                        LabeledInput {
                            id,
                            label,
                            value: value.clone(),
                            step: Some(param.step_value()),
                            min: Some(param.min_value()),
                            onchange: use_on_pos_dist_input_change(
                                pos_dist_type.clone(),
                                param.clone(),
                                light_data_builder_sig.clone(),
                            ),
                            r#type: "number",
                        }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[component]
pub fn PositionDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayPositionDistributionSelector { light_data_builder_sig }
        RayDistributionEditor { light_data_builder_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Position Distribution",
            header_id: "sourcePositionDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourcePositionDistCollapse",
        }
    }
}

fn use_on_pos_dist_input_change(
    mut pos_dist_type: PosDistType,
    param: PosDistParam,
    mut light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Option<Callback<Event<FormData>>> {
    Some(use_callback(move |e: Event<FormData>| {
        let value = e.value();
        if let Ok(value) = value.parse::<f64>() {
            match &mut pos_dist_type {
                PosDistType::Random(random) => match param {
                    PosDistParam::LengthX => random.set_side_length_x(millimeter!(value)),
                    PosDistParam::LengthY => random.set_side_length_y(millimeter!(value)),
                    PosDistParam::PointsX => random.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::Grid(grid) => match param {
                    PosDistParam::LengthX => grid.set_side_length_x(millimeter!(value)),
                    PosDistParam::LengthY => grid.set_side_length_y(millimeter!(value)),
                    PosDistParam::PointsX => grid.set_nr_of_points_x(value as usize),
                    PosDistParam::PointsY => grid.set_nr_of_points_y(value as usize),
                    _ => {}
                },
                PosDistType::HexagonalTiling(hexagonal_tiling) => match param {
                    PosDistParam::Rings => hexagonal_tiling.set_nr_of_hex_along_radius(value as u8),
                    PosDistParam::Radius => hexagonal_tiling.set_radius(millimeter!(value)),
                    PosDistParam::CenterX => hexagonal_tiling.set_center_x(millimeter!(value)),
                    PosDistParam::CenterY => hexagonal_tiling.set_center_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::Hexapolar(hexapolar) => match param {
                    PosDistParam::Rings => hexapolar.set_nr_of_rings(value as u8),
                    PosDistParam::Radius => hexapolar.set_radius(millimeter!(value)),
                    _ => {}
                },
                PosDistType::FibonacciRectangle(rect) => match param {
                    PosDistParam::LengthX => rect.set_side_length_x(millimeter!(value)),
                    PosDistParam::LengthY => rect.set_side_length_y(millimeter!(value)),
                    PosDistParam::PointsX => rect.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::FibonacciEllipse(ellipse) => match param {
                    PosDistParam::LengthX => ellipse.set_radius_x(millimeter!(value)),
                    PosDistParam::LengthY => ellipse.set_radius_y(millimeter!(value)),
                    PosDistParam::PointsX => ellipse.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::Sobol(sobol_dist) => match param {
                    PosDistParam::PointsX => sobol_dist.set_nr_of_points(value as usize),
                    PosDistParam::LengthX => sobol_dist.set_side_length_x(millimeter!(value)),
                    PosDistParam::LengthY => sobol_dist.set_side_length_y(millimeter!(value)),
                    _ => {}
                },
            }
            light_data_builder_sig.with_mut(|ldb| ldb.set_pos_dist_type(pos_dist_type))
        }
    }))
}

impl TryFrom<LightDataBuilder> for PosDistSelection {
    type Error = String;

    fn try_from(value: LightDataBuilder) -> Result<Self, Self::Error> {
        match value {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Ok(Self::new(collimated_src.pos_dist().clone()))
                }
                RayDataBuilder::PointSrc(point_src) => Ok(Self::new(point_src.pos_dist().clone())),
                RayDataBuilder::Raw(rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image {
                    file_path,
                    pixel_size,
                    total_energy,
                    wave_length,
                    cone_angle,
                } => Err("not used yet".to_owned()),
            },
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}

#[derive(Clone, PartialEq)]
struct PosDistSelection {
    pub pos_dist: PosDistType,
    pub rand: bool,
    pub grid: bool,
    pub hexagonal: bool,
    pub hexapolar: bool,
    pub fibonacci_rect: bool,
    pub fibonacci_ell: bool,
    pub sobol: bool,
}

impl PosDistSelection {
    pub fn new(pos_dist: PosDistType) -> Self {
        let mut select = Self {
            pos_dist: pos_dist.clone(),
            rand: false,
            grid: false,
            hexagonal: false,
            hexapolar: false,
            fibonacci_rect: false,
            fibonacci_ell: false,
            sobol: false,
        };

        select.set_dist(pos_dist);
        select
    }
    pub fn set_dist(&mut self, pos_dist: PosDistType) {
        (
            self.rand,
            self.grid,
            self.hexagonal,
            self.hexapolar,
            self.fibonacci_rect,
            self.fibonacci_ell,
            self.sobol,
        ) = match pos_dist {
            PosDistType::Random(_) => (true, false, false, false, false, false, false),
            PosDistType::Grid(_) => (false, true, false, false, false, false, false),
            PosDistType::HexagonalTiling(_) => (false, false, true, false, false, false, false),
            PosDistType::Hexapolar(_) => (false, false, false, true, false, false, false),
            PosDistType::FibonacciRectangle(_) => (false, false, false, false, true, false, false),
            PosDistType::FibonacciEllipse(_) => (false, false, false, false, false, true, false),
            PosDistType::Sobol(_) => (false, false, false, false, false, false, true),
        };

        self.pos_dist = pos_dist;
    }

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for pos_dist in PosDistType::iter() {
            option_vals.push(match pos_dist {
                PosDistType::Random(_) => (self.rand, pos_dist.to_string()),
                PosDistType::Grid(_) => (self.grid, pos_dist.to_string()),
                PosDistType::HexagonalTiling(_) => (self.hexagonal, pos_dist.to_string()),
                PosDistType::Hexapolar(_) => (self.hexapolar, pos_dist.to_string()),
                PosDistType::FibonacciRectangle(_) => (self.fibonacci_rect, pos_dist.to_string()),
                PosDistType::FibonacciEllipse(_) => (self.fibonacci_ell, pos_dist.to_string()),
                PosDistType::Sobol(_) => (self.sobol, pos_dist.to_string()),
            })
        }
        option_vals
    }
}
