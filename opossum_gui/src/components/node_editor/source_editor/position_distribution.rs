use std::fmt::Display;

use crate::components::node_editor::{
    accordion::{AccordionItem, LabeledSelect},
    source_editor::{DistInput, LightDataBuilderHistory, RowedDistInputs},
};
use dioxus::prelude::*;
use opossum_backend::{
    light_data_builder::LightDataBuilder, millimeter, ray_data_builder::RayDataBuilder, PosDistType,
};
use strum::IntoEnumIterator;
use uom::si::length::millimeter;

#[derive(Clone, PartialEq, Copy)]
pub enum DistParam {
    Rings,
    Radius,
    CenterX,
    CenterY,
    LengthX,
    LengthY,
    PointsX,
    PointsY,
    Energy,
    Angle,
    Power,
    Rectangular,
    WaveLengthStart,
    WaveLengthEnd,
    WaveLength,
    FWHM,
    RelIntensity,
}

impl DistParam {
    pub fn input_label(&self) -> String {
        match self {
            DistParam::Rings => "Number of Rings".to_string(),
            DistParam::Radius => "Radius in mm".to_string(),
            DistParam::CenterX => "Center X in mm".to_string(),
            DistParam::CenterY => "Center Y in mm".to_string(),
            DistParam::LengthX => "Length X in mm".to_string(),
            DistParam::LengthY => "Length Y in mm".to_string(),
            DistParam::PointsX => "#Points X".to_string(),
            DistParam::PointsY => "#Points Y".to_string(),
            DistParam::Energy => "Energy in J".to_string(),
            DistParam::Angle => "Angle in degree".to_string(),
            DistParam::Power => "Power".to_string(),
            DistParam::Rectangular => "Rectangular".to_string(),
            DistParam::WaveLengthStart => "Start λ in nm".to_string(),
            DistParam::WaveLengthEnd => "End λ in nm".to_string(),
            DistParam::WaveLength => "λ in nm".to_string(),
            DistParam::FWHM => "FWHM in nm".to_string(),
            DistParam::RelIntensity => "Rel. intensity".to_string(),
        }
    }

    pub fn min_value(&self) -> Option<&'static str> {
        match self {
            DistParam::Rings | DistParam::PointsX | DistParam::PointsY => Some("1"),
            DistParam::Radius
            | DistParam::LengthX
            | DistParam::LengthY
            | DistParam::Angle
            | DistParam::Power
            | DistParam::WaveLengthStart
            | DistParam::WaveLengthEnd
            | DistParam::FWHM
            | DistParam::WaveLength => Some("1e-9"),
            DistParam::CenterX | DistParam::CenterY => Some("-1e9"),
            DistParam::Energy | DistParam::RelIntensity => Some("0."),
            _ => None,
        }
    }
    pub fn step_value(&self) -> Option<&'static str> {
        match self {
            DistParam::Rectangular => None,
            _ => Some("1"),
        }
    }
}

impl Display for DistParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param = match self {
            DistParam::Rings => "Rings",
            DistParam::Radius => "Radius",
            DistParam::CenterX => "CenterX",
            DistParam::CenterY => "CenterY",
            DistParam::LengthX => "LengthX",
            DistParam::LengthY => "LengthY",
            DistParam::PointsX => "PointsX",
            DistParam::PointsY => "PointsY",
            DistParam::Energy => "Energy",
            DistParam::Angle => "Angle",
            DistParam::Power => "Power",
            DistParam::Rectangular => "Rectangular",
            DistParam::WaveLengthStart => "StartWavelength",
            DistParam::WaveLengthEnd => "EndWavelength",
            DistParam::WaveLength => "Wavelength",
            DistParam::FWHM => "FWHM",
            DistParam::RelIntensity => "Relativeintensity",
        };
        write!(f, "{param}")
    }
}

fn get_pos_dist_input_params(
    pos_dist_type: PosDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Vec<DistInput> {
    let mut dist_inputs: Vec<DistInput> = match pos_dist_type {
        PosDistType::Random(random) => vec![
            DistInput::new(
                DistParam::LengthX,
                &pos_dist_type,
                None,
                format!("{}", random.side_length_x().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthY,
                &pos_dist_type,
                None,
                format!("{}", random.side_length_y().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::PointsX,
                &pos_dist_type,
                None,
                format!("{}", random.nr_of_points()),
            ),
        ],
        PosDistType::Grid(grid) => vec![
            DistInput::new(
                DistParam::LengthX,
                &pos_dist_type,
                None,
                format!("{}", grid.side_length().0.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthY,
                &pos_dist_type,
                None,
                format!("{}", grid.side_length().1.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::PointsX,
                &pos_dist_type,
                None,
                format!("{}", grid.nr_of_points().0),
            ),
            DistInput::new(
                DistParam::PointsY,
                &pos_dist_type,
                None,
                format!("{}", grid.nr_of_points().1),
            ),
        ],
        PosDistType::HexagonalTiling(hexagonal_tiling) => vec![
            DistInput::new(
                DistParam::Rings,
                &pos_dist_type,
                None,
                format!("{}", hexagonal_tiling.nr_of_hex_along_radius()),
            ),
            DistInput::new(
                DistParam::Radius,
                &pos_dist_type,
                None,
                format!("{}", hexagonal_tiling.radius().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::CenterX,
                &pos_dist_type,
                None,
                format!("{}", hexagonal_tiling.center().x.get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::CenterY,
                &pos_dist_type,
                None,
                format!("{}", hexagonal_tiling.center().y.get::<millimeter>()),
            ),
        ],
        PosDistType::Hexapolar(hexapolar) => vec![
            DistInput::new(
                DistParam::Rings,
                &pos_dist_type,
                None,
                format!("{}", hexapolar.nr_of_rings()),
            ),
            DistInput::new(
                DistParam::Radius,
                &pos_dist_type,
                None,
                format!("{}", hexapolar.radius().get::<millimeter>()),
            ),
        ],
        PosDistType::FibonacciRectangle(fibonacci_rectangle) => vec![
            DistInput::new(
                DistParam::LengthX,
                &pos_dist_type,
                None,
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_x().get::<millimeter>()
                ),
            ),
            DistInput::new(
                DistParam::LengthY,
                &pos_dist_type,
                None,
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_y().get::<millimeter>()
                ),
            ),
            DistInput::new(
                DistParam::PointsX,
                &pos_dist_type,
                None,
                format!("{}", fibonacci_rectangle.nr_of_points()),
            ),
        ],
        PosDistType::FibonacciEllipse(fibonacci_ellipse) => vec![
            DistInput::new(
                DistParam::LengthX,
                &pos_dist_type,
                None,
                format!("{}", fibonacci_ellipse.radius_x().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthY,
                &pos_dist_type,
                None,
                format!("{}", fibonacci_ellipse.radius_y().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::PointsX,
                &pos_dist_type,
                None,
                format!("{}", fibonacci_ellipse.nr_of_points()),
            ),
        ],
        PosDistType::Sobol(sobol_dist) => vec![
            DistInput::new(
                DistParam::LengthX,
                &pos_dist_type,
                None,
                format!("{}", sobol_dist.side_length_x().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::LengthY,
                &pos_dist_type,
                None,
                format!("{}", sobol_dist.side_length_y().get::<millimeter>()),
            ),
            DistInput::new(
                DistParam::PointsX,
                &pos_dist_type,
                None,
                format!("{}", sobol_dist.nr_of_points()),
            ),
        ],
    };

    for dist_input in &mut dist_inputs {
        dist_input.callback_opt = use_on_pos_dist_input_change(
            pos_dist_type,
            dist_input.dist_param,
            light_data_builder_sig,
        );
    }

    dist_inputs
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
pub fn RayPosDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist = light_data_builder_sig.read().get_current_pos_dist_type();

    rsx! {
        div { hidden: !show,
            {
                if let Some(pos_dist_type) = rays_pos_dist {
                    rsx! {
                        NodePosDistInputs { pos_dist_type, light_data_builder_sig }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[component]
pub fn NodePosDistInputs(
    pos_dist_type: PosDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let dist_params = get_pos_dist_input_params(pos_dist_type, light_data_builder_sig);
    rsx! {
        RowedDistInputs { dist_params }
    }
}

#[component]
pub fn PositionDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayPositionDistributionSelector { light_data_builder_sig }
        RayPosDistributionEditor { light_data_builder_sig }
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
    param: DistParam,
    mut light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Option<Callback<Event<FormData>>> {
    Some(use_callback(move |e: Event<FormData>| {
        let value = e.value();
        if let Ok(value) = value.parse::<f64>() {
            match &mut pos_dist_type {
                PosDistType::Random(random) => match param {
                    DistParam::LengthX => random.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => random.set_side_length_y(millimeter!(value)),
                    DistParam::PointsX => random.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::Grid(grid) => match param {
                    DistParam::LengthX => grid.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => grid.set_side_length_y(millimeter!(value)),
                    DistParam::PointsX => grid.set_nr_of_points_x(value as usize),
                    DistParam::PointsY => grid.set_nr_of_points_y(value as usize),
                    _ => {}
                },
                PosDistType::HexagonalTiling(hexagonal_tiling) => match param {
                    DistParam::Rings => hexagonal_tiling.set_nr_of_hex_along_radius(value as u8),
                    DistParam::Radius => hexagonal_tiling.set_radius(millimeter!(value)),
                    DistParam::CenterX => hexagonal_tiling.set_center_x(millimeter!(value)),
                    DistParam::CenterY => hexagonal_tiling.set_center_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::Hexapolar(hexapolar) => match param {
                    DistParam::Rings => hexapolar.set_nr_of_rings(value as u8),
                    DistParam::Radius => hexapolar.set_radius(millimeter!(value)),
                    _ => {}
                },
                PosDistType::FibonacciRectangle(rect) => match param {
                    DistParam::LengthX => rect.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => rect.set_side_length_y(millimeter!(value)),
                    DistParam::PointsX => rect.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::FibonacciEllipse(ellipse) => match param {
                    DistParam::LengthX => ellipse.set_radius_x(millimeter!(value)),
                    DistParam::LengthY => ellipse.set_radius_y(millimeter!(value)),
                    DistParam::PointsX => ellipse.set_nr_of_points(value as usize),
                    _ => {}
                },
                PosDistType::Sobol(sobol_dist) => match param {
                    DistParam::PointsX => sobol_dist.set_nr_of_points(value as usize),
                    DistParam::LengthX => sobol_dist.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => sobol_dist.set_side_length_y(millimeter!(value)),
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
                    Ok(Self::new(*collimated_src.pos_dist()))
                }
                RayDataBuilder::PointSrc(point_src) => Ok(Self::new(*point_src.pos_dist())),
                RayDataBuilder::Raw(_rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image { .. } => Err("not used yet".to_owned()),
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
            pos_dist,
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
