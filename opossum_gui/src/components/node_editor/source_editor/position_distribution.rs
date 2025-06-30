#![allow(clippy::derive_partial_eq_without_eq)]
use std::fmt::Display;

use crate::{
    components::node_editor::{
        accordion::{AccordionItem, LabeledSelect},
        source_editor::{DistInput, LightDataBuilderHistory, RowedDistInputs},
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{
    light_data_builder::LightDataBuilder, millimeter, ray_data_builder::RayDataBuilder,
    FibonacciEllipse, FibonacciRectangle, Grid, HexagonalTiling, Hexapolar, PosDistType, Random,
    SobolDist,
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
    #[must_use]
    pub fn input_label(&self) -> String {
        match self {
            Self::Rings => "Number of Rings".to_string(),
            Self::Radius => "Radius in mm".to_string(),
            Self::CenterX => "Center X in mm".to_string(),
            Self::CenterY => "Center Y in mm".to_string(),
            Self::LengthX => "Length X in mm".to_string(),
            Self::LengthY => "Length Y in mm".to_string(),
            Self::PointsX => "#Points X".to_string(),
            Self::PointsY => "#Points Y".to_string(),
            Self::Energy => "Energy in J".to_string(),
            Self::Angle => "Angle in degree".to_string(),
            Self::Power => "Power".to_string(),
            Self::Rectangular => "Rectangular".to_string(),
            Self::WaveLengthStart => "Start λ in nm".to_string(),
            Self::WaveLengthEnd => "End λ in nm".to_string(),
            Self::WaveLength => "λ in nm".to_string(),
            Self::FWHM => "FWHM in nm".to_string(),
            Self::RelIntensity => "Rel. intensity".to_string(),
        }
    }

    #[must_use]
    pub const fn min_value(&self) -> Option<&'static str> {
        match self {
            Self::Rings | Self::PointsX | Self::PointsY => Some("1"),
            Self::Radius
            | Self::LengthX
            | Self::LengthY
            | Self::Angle
            | Self::Power
            | Self::WaveLengthStart
            | Self::WaveLengthEnd
            | Self::FWHM
            | Self::WaveLength => Some("1e-9"),
            Self::CenterX | Self::CenterY => Some("-1e9"),
            Self::Energy | Self::RelIntensity => Some("0."),
            Self::Rectangular => None,
        }
    }
    #[must_use]
    pub const fn step_value(&self) -> Option<&'static str> {
        match self {
            Self::Rectangular => None,
            _ => Some("1"),
        }
    }
}

impl Display for DistParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param = match self {
            Self::Rings => "Rings",
            Self::Radius => "Radius",
            Self::CenterX => "CenterX",
            Self::CenterY => "CenterY",
            Self::LengthX => "LengthX",
            Self::LengthY => "LengthY",
            Self::PointsX => "PointsX",
            Self::PointsY => "PointsY",
            Self::Energy => "Energy",
            Self::Angle => "Angle",
            Self::Power => "Power",
            Self::Rectangular => "Rectangular",
            Self::WaveLengthStart => "StartWavelength",
            Self::WaveLengthEnd => "EndWavelength",
            Self::WaveLength => "Wavelength",
            Self::FWHM => "FWHM",
            Self::RelIntensity => "Relativeintensity",
        };
        write!(f, "{param}")
    }
}

fn get_pos_dist_input_params(
    pos_dist_type: PosDistType,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Vec<DistInput> {
    let mut dist_inputs: Vec<DistInput> = match &pos_dist_type {
        PosDistType::Random(r) => get_random_dist_input_params(r, &pos_dist_type),
        PosDistType::Grid(g) => get_grid_dist_input_params(g, &pos_dist_type),
        PosDistType::HexagonalTiling(h) => get_hexagonal_dist_input_params(h, &pos_dist_type),
        PosDistType::Hexapolar(hp) => get_hexapolar_dist_input_params(hp, &pos_dist_type),
        PosDistType::FibonacciRectangle(fr) => {
            get_fibonacci_rect_dist_input_params(fr, &pos_dist_type)
        }
        PosDistType::FibonacciEllipse(fe) => {
            get_fibonacci_ellipse_dist_input_params(fe, &pos_dist_type)
        }
        PosDistType::Sobol(s) => get_sobol_dist_input_params(s, &pos_dist_type),
    };

    for dist_input in &mut dist_inputs {
        dist_input.callback_opt = Some(use_on_pos_dist_input_change(
            pos_dist_type,
            dist_input.dist_param,
            light_data_builder_sig,
        ));
    }

    dist_inputs
}

fn get_random_dist_input_params(random: &Random, dist_type: &PosDistType) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::LengthX,
            dist_type,
            None,
            format!("{}", random.side_length_x().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::LengthY,
            dist_type,
            None,
            format!("{}", random.side_length_y().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::PointsX,
            dist_type,
            None,
            format!("{}", random.nr_of_points()),
        ),
    ]
}

fn get_grid_dist_input_params(grid: &Grid, dist_type: &PosDistType) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::LengthX,
            dist_type,
            None,
            format!("{}", grid.side_length().0.get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::LengthY,
            dist_type,
            None,
            format!("{}", grid.side_length().1.get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::PointsX,
            dist_type,
            None,
            format!("{}", grid.nr_of_points().0),
        ),
        DistInput::new(
            DistParam::PointsY,
            dist_type,
            None,
            format!("{}", grid.nr_of_points().1),
        ),
    ]
}

fn get_hexagonal_dist_input_params(
    hex: &HexagonalTiling,
    dist_type: &PosDistType,
) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::Rings,
            dist_type,
            None,
            format!("{}", hex.nr_of_hex_along_radius()),
        ),
        DistInput::new(
            DistParam::Radius,
            dist_type,
            None,
            format!("{}", hex.radius().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::CenterX,
            dist_type,
            None,
            format!("{}", hex.center().x.get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::CenterY,
            dist_type,
            None,
            format!("{}", hex.center().y.get::<millimeter>()),
        ),
    ]
}

fn get_hexapolar_dist_input_params(
    hexapolar: &Hexapolar,
    dist_type: &PosDistType,
) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::Rings,
            dist_type,
            None,
            format!("{}", hexapolar.nr_of_rings()),
        ),
        DistInput::new(
            DistParam::Radius,
            dist_type,
            None,
            format!("{}", hexapolar.radius().get::<millimeter>()),
        ),
    ]
}

fn get_fibonacci_rect_dist_input_params(
    fr: &FibonacciRectangle,
    dist_type: &PosDistType,
) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::LengthX,
            dist_type,
            None,
            format!("{}", fr.side_length_x().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::LengthY,
            dist_type,
            None,
            format!("{}", fr.side_length_y().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::PointsX,
            dist_type,
            None,
            format!("{}", fr.nr_of_points()),
        ),
    ]
}

fn get_fibonacci_ellipse_dist_input_params(
    fe: &FibonacciEllipse,
    dist_type: &PosDistType,
) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::LengthX,
            dist_type,
            None,
            format!("{}", fe.radius_x().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::LengthY,
            dist_type,
            None,
            format!("{}", fe.radius_y().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::PointsX,
            dist_type,
            None,
            format!("{}", fe.nr_of_points()),
        ),
    ]
}

fn get_sobol_dist_input_params(sobol: &SobolDist, dist_type: &PosDistType) -> Vec<DistInput> {
    vec![
        DistInput::new(
            DistParam::LengthX,
            dist_type,
            None,
            format!("{}", sobol.side_length_x().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::LengthY,
            dist_type,
            None,
            format!("{}", sobol.side_length_y().get::<millimeter>()),
        ),
        DistInput::new(
            DistParam::PointsX,
            dist_type,
            None,
            format!("{}", sobol.nr_of_points()),
        ),
    ]
}

#[component]
pub fn RayPositionDistributionSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist = PosDistSelection::try_from(light_data_builder_sig.read().get_current());

    rays_pos_dist.map_or_else(
        |_| rsx! {},
        |rpd| {
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
        },
    )
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
                 rays_pos_dist.map_or_else(|| rsx! {}, |pos_dist_type| rsx! {
                         NodePosDistInputs { pos_dist_type, light_data_builder_sig }
                     })
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
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        let value = e.value();
        if let Ok(value) = value.parse::<u8>() {
            match &mut pos_dist_type {
                PosDistType::HexagonalTiling(hexagonal_tiling) => {
                    if param == DistParam::Rings {
                        hexagonal_tiling.set_nr_of_hex_along_radius(value);
                    }
                }
                PosDistType::Hexapolar(hexapolar) => {
                    if param == DistParam::Rings {
                        hexapolar.set_nr_of_rings(value);
                    }
                }
                _ => {}
            }
        } else if let Ok(value) = value.parse::<usize>() {
            match &mut pos_dist_type {
                PosDistType::Random(random) => {
                    if param == DistParam::PointsX {
                        random.set_nr_of_points(value);
                    }
                }
                PosDistType::Grid(grid) => match param {
                    DistParam::PointsX => grid.set_nr_of_points_x(value),
                    DistParam::PointsY => grid.set_nr_of_points_y(value),
                    _ => {}
                },
                PosDistType::FibonacciRectangle(rect) => {
                    if param == DistParam::PointsX {
                        rect.set_nr_of_points(value);
                    }
                }
                PosDistType::FibonacciEllipse(ellipse) => {
                    if param == DistParam::PointsX {
                        ellipse.set_nr_of_points(value);
                    }
                }
                PosDistType::Sobol(sobol_dist) => {
                    if param == DistParam::PointsX {
                        sobol_dist.set_nr_of_points(value);
                    }
                }
                _ => {}
            }
        } else if let Ok(value) = value.parse::<f64>() {
            match &mut pos_dist_type {
                PosDistType::Random(random) => match param {
                    DistParam::LengthX => random.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => random.set_side_length_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::Grid(grid) => match param {
                    DistParam::LengthX => grid.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => grid.set_side_length_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::HexagonalTiling(hexagonal_tiling) => match param {
                    DistParam::Radius => hexagonal_tiling.set_radius(millimeter!(value)),
                    DistParam::CenterX => hexagonal_tiling.set_center_x(millimeter!(value)),
                    DistParam::CenterY => hexagonal_tiling.set_center_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::Hexapolar(hexapolar) => {
                    if param == DistParam::Radius {
                        hexapolar.set_radius(millimeter!(value));
                    }
                }
                PosDistType::FibonacciRectangle(rect) => match param {
                    DistParam::LengthX => rect.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => rect.set_side_length_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::FibonacciEllipse(ellipse) => match param {
                    DistParam::LengthX => ellipse.set_radius_x(millimeter!(value)),
                    DistParam::LengthY => ellipse.set_radius_y(millimeter!(value)),
                    _ => {}
                },
                PosDistType::Sobol(sobol_dist) => match param {
                    DistParam::LengthX => sobol_dist.set_side_length_x(millimeter!(value)),
                    DistParam::LengthY => sobol_dist.set_side_length_y(millimeter!(value)),
                    _ => {}
                },
            }
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Unable to parse passed value, please check input parameters!");
        }
        light_data_builder_sig.with_mut(|ldb| ldb.set_pos_dist_type(pos_dist_type));
    })
}

impl TryFrom<Option<&LightDataBuilder>> for PosDistSelection {
    type Error = String;

    fn try_from(value: Option<&LightDataBuilder>) -> Result<Self, Self::Error> {
        match value {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
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
#[allow(clippy::struct_excessive_bools)]
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
    pub const fn new(pos_dist: PosDistType) -> Self {
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
    pub const fn set_dist(&mut self, pos_dist: PosDistType) {
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
            });
        }
        option_vals
    }
}
