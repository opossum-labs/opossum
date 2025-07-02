use std::fmt::Display;

use crate::components::node_editor::CallbackWrapper;

#[derive(Clone, PartialEq, Copy)]
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
    FWHM,
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
}

impl InputParam {
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
            Self::PixelSize => "Pixel size in µm".to_string(),
            Self::FilePath => "File".to_string(),
            Self::ConeAngle => "Cone angle in degrees".to_string(),
            Self::TranslationX => "X Translation in mm".to_string(),
            Self::TranslationY => "Y Translation in mm".to_string(),
            Self::TranslationZ => "Z Translation in mm".to_string(),
            Self::RotationRoll => "Roll in degrees".to_string(),
            Self::RotationPitch => "Pitch in degrees".to_string(),
            Self::RotationYaw => "Yaw in degrees".to_string(),
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
            | Self::WaveLength
            | Self::ConeAngle
            | Self::PixelSize => Some("1e-9"),
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
    pub const fn step_value(&self) -> Option<&'static str> {
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
            Self::FWHM => "FWHM",
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
