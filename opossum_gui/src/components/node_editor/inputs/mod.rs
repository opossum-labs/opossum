pub mod input_components;

use opossum_backend::{RotationAxis, TranslationAxis};

use crate::components::node_editor::CallbackWrapper;
use std::fmt::Display;

#[derive(Clone, PartialEq, Copy, Eq)]
pub enum InputParam {
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
    Fwhm,
    RelIntensity,
    PixelSize,
    FilePath,
    ConeAngle,
    TranslationX,
    TranslationY,
    TranslationZ,
    RotationRoll,
    RotationPitch,
    RotationYaw,
    RefractiveIndex,
    Sellmeierk1,
    Sellmeierk2,
    Sellmeierk3,
    Sellmeierl1,
    Sellmeierl2,
    Sellmeierl3,
    Schott0,
    Schott1,
    Schott2,
    Schott3,
    Schott4,
    Schott5,
    Conrady0,
    Conrady1,
    Conrady2,
}

impl From<TranslationAxis> for InputParam {
    fn from(axis: TranslationAxis) -> Self {
        match axis {
            TranslationAxis::X => Self::TranslationX,
            TranslationAxis::Y => Self::TranslationY,
            TranslationAxis::Z => Self::TranslationZ,
        }
    }
}

impl From<RotationAxis> for InputParam {
    fn from(axis: RotationAxis) -> Self {
        match axis {
            RotationAxis::Roll => Self::RotationRoll,
            RotationAxis::Pitch => Self::RotationPitch,
            RotationAxis::Yaw => Self::RotationYaw,
        }
    }
}

impl InputParam {
    #[must_use]
    pub fn input_label(self) -> String {
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
            Self::Fwhm => "FWHM in nm".to_string(),
            Self::RelIntensity => "Rel. intensity".to_string(),
            Self::PixelSize => "Pixel size in µm".to_string(),
            Self::FilePath => "File".to_string(),
            Self::ConeAngle => "Cone angle in degrees".to_string(),
            Self::TranslationX => "X Translation in mm".to_string(),
            Self::TranslationY => "Y Translation in mm".to_string(),
            Self::TranslationZ => "Z Translation in mm".to_string(),
            Self::RotationRoll => "Roll in degrees".to_string(),
            Self::RotationPitch => "Pitch in degrees".to_string(),
            Self::RotationYaw => "Yaw in degrees".to_string(),
            Self::RefractiveIndex => "Refractive index".to_string(),
            Self::Sellmeierk1 => "B1".to_string(),
            Self::Sellmeierk2 => "B2".to_string(),
            Self::Sellmeierk3 => "B3".to_string(),
            Self::Sellmeierl1 => "C1".to_string(),
            Self::Sellmeierl2 => "C2".to_string(),
            Self::Sellmeierl3 => "C3".to_string(),
            Self::Schott0 | Self::Conrady0 => "A".to_string(),
            Self::Schott1 | Self::Conrady1 => "B".to_string(),
            Self::Schott2 | Self::Conrady2 => "C".to_string(),
            Self::Schott3 => "D".to_string(),
            Self::Schott4 => "E".to_string(),
            Self::Schott5 => "F".to_string(),
        }
    }

    #[must_use]
    pub const fn min_value(self) -> Option<&'static str> {
        match self {
            Self::Rings | Self::PointsX | Self::PointsY | Self::RefractiveIndex => Some("1"),
            Self::Radius
            | Self::LengthX
            | Self::LengthY
            | Self::Angle
            | Self::Power
            | Self::WaveLengthStart
            | Self::WaveLengthEnd
            | Self::Fwhm
            | Self::WaveLength
            | Self::ConeAngle
            | Self::PixelSize
            | Self::Sellmeierk1
            | Self::Sellmeierk2
            | Self::Sellmeierk3
            | Self::Sellmeierl1
            | Self::Sellmeierl2
            | Self::Sellmeierl3
            | Self::Schott0
            | Self::Schott1
            | Self::Schott2
            | Self::Schott3
            | Self::Schott4
            | Self::Schott5
            | Self::Conrady0
            | Self::Conrady1
            | Self::Conrady2 => Some("1e-9"),
            Self::CenterX
            | Self::CenterY
            | Self::TranslationX
            | Self::TranslationY
            | Self::TranslationZ
            | Self::RotationRoll
            | Self::RotationPitch
            | Self::RotationYaw => Some("-1e9"),
            Self::Energy | Self::RelIntensity => Some("0."),
            Self::Rectangular | Self::FilePath => None,
        }
    }
    #[must_use]
    pub const fn step_value(self) -> Option<&'static str> {
        match self {
            Self::Rectangular | Self::FilePath => None,
            _ => Some("1"),
        }
    }
}

impl Display for InputParam {
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
            Self::Fwhm => "FWHM",
            Self::RelIntensity => "Relativeintensity",
            Self::PixelSize => "PixelSize",
            Self::FilePath => "FilePath",
            Self::ConeAngle => "ConeAngle",
            Self::TranslationX => "TransX",
            Self::TranslationY => "TransY",
            Self::TranslationZ => "TransZ",
            Self::RotationRoll => "RollAngle",
            Self::RotationPitch => "PitchAngle",
            Self::RotationYaw => "YawAngle",
            Self::RefractiveIndex => "RefractiveIndex",
            Self::Sellmeierk1 => "Sellmeierk1",
            Self::Sellmeierk2 => "Sellmeierk2",
            Self::Sellmeierk3 => "Sellmeierk3",
            Self::Sellmeierl1 => "Sellmeierl1",
            Self::Sellmeierl2 => "Sellmeierl2",
            Self::Sellmeierl3 => "Sellmeierl3",
            Self::Schott0 => "Schotta0",
            Self::Schott1 => "Schotta1",
            Self::Schott2 => "Schotta2",
            Self::Schott3 => "Schotta3",
            Self::Schott4 => "Schotta4",
            Self::Schott5 => "Schotta5",
            Self::Conrady0 => "Conrady0",
            Self::Conrady1 => "Conrady1",
            Self::Conrady2 => "Conrady2",
        };
        write!(f, "{param}")
    }
}

#[derive(Clone, PartialEq)]
pub struct InputData {
    pub value: String,
    pub id: String,
    pub dist_param: InputParam,
    pub callback_opt: CallbackWrapper,
}

impl InputData {
    pub fn new(
        dist_param: InputParam,
        dist_type: &impl Display,
        callback_opt: CallbackWrapper,
        value: String,
    ) -> Self {
        Self {
            value,
            id: format!("node{dist_type}{dist_param}Input"),
            dist_param,
            callback_opt,
        }
    }
}
