use crate::{
    aperture::Aperture,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::{
        fluence_detector::Fluence, ray_propagation_visualizer::RayPositionHistories,
        reflective_grating::LinearDensity, FilterType, FluenceData, Metertype, Spectrometer,
        SpectrometerType, SpotDiagram, WaveFrontData,
    },
    optic_graph::OpticGraph,
    optic_ports::OpticPorts,
    ray::SplittingConfig,
    refractive_index::RefractiveIndexType,
    reporter::{HtmlNodeReport, NodeReport},
    utils::{geom_transformation::Isometry, EnumProxy},
};
use nalgebra::Vector3;
use num::Float;
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::meter,
    radiant_exposure::joule_per_square_centimeter,
    Dimension, Quantity, Unit, Units,
};
use uom::{lib::fmt::Debug, si::f64::Angle};
use uuid::Uuid;
static HTML_PROP_SIMPLE: &str = include_str!("../html/prop_simple.html");
static HTML_PROP_IMAGE: &str = include_str!("../html/prop_image.html");
static HTML_PROP_GROUP: &str = include_str!("../html/node_report.html");

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
/// The type of the [`Property`](crate::properties::Property).
pub enum Proptype {
    /// A string property
    ///
    /// This property makes use of [`PropCondition::NonEmptyString`] for restricting strings to be non-empty.
    String(String),
    /// An integer property
    ///
    /// This property respects the [`PropCondition::LessThan`], [`PropCondition::LessThanEqual`], [`PropCondition::GreaterThan`], and [`PropCondition::GreaterThanEqual`]
    I32(i32),
    /// A float property
    ///
    /// This property respects the [`PropCondition::LessThan`], [`PropCondition::LessThanEqual`], [`PropCondition::GreaterThan`], and [`PropCondition::GreaterThanEqual`]
    F64(f64),
    /// A boolean property
    Bool(bool),
    /// An optional [`LightData`] property
    LightData(EnumProxy<Option<LightData>>),
    /// A property for storing a complete `OpticGraph` to be used by [`OpticScenery`](crate::OpticScenery).
    OpticGraph(OpticGraph),
    /// Property for storing a [`FilterType`] of an [`IdealFilter`](crate::nodes::IdealFilter) node.
    FilterType(EnumProxy<FilterType>),
    /// Property for storing a [`SplittingConfig`] of an [`BeamSplitter`](crate::nodes::BeamSplitter) node.
    SplitterType(EnumProxy<SplittingConfig>),
    /// Property for storing a [`SpectrometerType`] of a [`Sepctrometer`](crate::nodes::Spectrometer) node.
    SpectrometerType(SpectrometerType),
    /// Property for storing a [`Metertype`] of an [`Energymeter`](crate::nodes::EnergyMeter) node.
    Metertype(Metertype),
    /// Property for storing the external port mapping (`PortMap`) of a [`Group`](crate::nodes::NodeGroup) node.
    //GroupPortMap(PortMap),
    /// An [`Uuid`] for identifying an optical node.
    Uuid(Uuid),
    /// A property for storing [`OpticPorts`].
    OpticPorts(OpticPorts),
    /// A property for storing an optical [`Aperture`].
    Aperture(Aperture),
    /// A property for storing a [`Spectrum`](crate::spectrum::Spectrum).
    Spectrometer(Spectrometer),
    /// This property stores optical [`Rays`](crate::rays::Rays)
    SpotDiagram(SpotDiagram),
    /// This property stores the fluence information [`FluenceData`]
    FluenceDetector(FluenceData),
    /// This property stores the wavefront Information [`WaveFrontData`]
    WaveFrontStats(WaveFrontData),
    /// This property stores the ray position history of all [`Rays`](crate::rays::Rays) during propagation through the optic scenery
    RayPositionHistory(RayPositionHistories),
    /// A (nested set) of Properties
    NodeReport(NodeReport),
    /// linear density in `1/length_unit`
    LinearDensity(LinearDensity),
    /// Fluence in Units of J/cm²
    Fluence(Fluence),
    /// Unit of Wavelength
    WfLambda(f64, Length),
    /// a geometrical length
    Length(Length),
    /// an energy value
    Energy(Energy),
    /// a optical refractive index model
    Angle(Angle),
    RefractiveIndex(EnumProxy<RefractiveIndexType>),
    /// a (node) location / orientation
    Isometry(EnumProxy<Option<Isometry>>),
    /// Three dimensional Vector
    Vec3(Vector3<f64>),
}
impl Proptype {
    /// Generate a html representation of a Proptype.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying html templates could not be compiled
    ///   - a property value could not be converted to html code.
    pub fn to_html(&self, property_name: &str, uuid: &str) -> OpmResult<String> {
        let mut tt = TinyTemplate::new();
        tt.add_template("simple", HTML_PROP_SIMPLE)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        tt.add_template("image", HTML_PROP_IMAGE)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        tt.add_template("group", HTML_PROP_GROUP)
            .map_err(|e| OpossumError::Other(e.to_string()))?;
        let string_value = match self {
            Self::String(value) => tt.render("simple", &value),
            Self::I32(value) => tt.render("simple", &format!("{value}")),
            Self::F64(value) => tt.render("simple", &format!("{value:.6}")),
            Self::Bool(value) => tt.render("simple", &format!("{value}")),
            Self::SpectrometerType(value) => tt.render("simple", &value.to_string()),
            Self::Metertype(value) => tt.render("simple", &value.to_string()),
            Self::Spectrometer(_) => tt.render(
                "image",
                &format!("data/spectrometer_{property_name}_{uuid}.svg"),
            ),
            Self::SpotDiagram(_) => tt.render(
                "image",
                &format!("data/spot_diagram_{property_name}_{uuid}.svg"),
            ),
            Self::WaveFrontStats(_value) => tt.render(
                "image",
                &format!("data/wavefront_diagram_{property_name}_{uuid}.png"),
            ),
            Self::FluenceDetector(_value) => {
                tt.render("image", &format!("data/fluence_{property_name}_{uuid}.png"))
            }
            Self::NodeReport(report) => {
                let html_node_report = HtmlNodeReport {
                    node: report.name().into(),
                    node_type: report.detector_type().into(),
                    props: report.properties().html_props(report.name(), uuid),
                    uuid: uuid.to_string(),
                };
                tt.render("group", &html_node_report)
            }
            Self::Fluence(value) => tt.render(
                "simple",
                &format_quantity(joule_per_square_centimeter, *value),
            ),
            Self::WfLambda(value, wvl) => tt.render(
                "simple",
                &format!(
                    "{}λ, (λ = {})",
                    format_value_with_prefix(*value,),
                    format_quantity(meter, *wvl)
                ),
            ),
            Self::Length(value) => tt.render("simple", &format_quantity(meter, *value)),
            Self::Energy(value) => tt.render("simple", &format_quantity(joule, *value)),
            Self::RayPositionHistory(_) => tt.render(
                "image",
                &format!("data/ray_propagation_{property_name}_{uuid}.svg"),
            ),
            _ => Ok("unknown property type".into()),
        };
        string_value.map_err(|e| OpossumError::Other(e.to_string()))
    }
}
impl From<bool> for Proptype {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<f64> for Proptype {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}
impl From<String> for Proptype {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}
impl From<&str> for Proptype {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}
impl From<i32> for Proptype {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}
impl From<Uuid> for Proptype {
    fn from(value: Uuid) -> Self {
        Self::Uuid(value)
    }
}
impl From<Length> for Proptype {
    fn from(value: Length) -> Self {
        Self::Length(value)
    }
}
impl From<Energy> for Proptype {
    fn from(value: Energy) -> Self {
        Self::Energy(value)
    }
}
impl From<Angle> for Proptype {
    fn from(value: Angle) -> Self {
        Self::Angle(value)
    }
}

fn format_value_with_prefix(value: f64) -> String {
    if value.is_nan() {
        return String::from("     nan ");
    }
    if value == f64::INFINITY {
        return String::from("     inf ");
    }
    if value == f64::NEG_INFINITY {
        return String::from("    -inf ");
    }
    if value.abs() < f64::EPSILON {
        return String::from("   0.000 ");
    }
    #[allow(clippy::cast_possible_truncation)]
    let mut exponent = (f64::log10(value.abs()).floor()) as i32;
    if exponent.is_negative() {
        exponent -= 2;
    }
    let exponent = (exponent / 3) * 3;
    let prefix = match exponent {
        -21 => "z",
        -18 => "a",
        -15 => "f",
        -12 => "p",
        -9 => "n",
        -6 => "μ",
        -3 => "m",
        0 => "",
        3 => "k",
        6 => "M",
        9 => "G",
        12 => "T",
        15 => "P",
        18 => "E",
        21 => "Z",
        _ => "?",
    };
    format!("{:8.3} {prefix}", value / f64::powi(10.0, exponent))
}
fn format_quantity<D, U, V, N>(_: N, q: Quantity<D, U, V>) -> String
where
    D: Dimension + ?Sized,
    U: Units<V> + ?Sized,
    V: Float + uom::Conversion<V> + Debug,
    N: Unit,
{
    let base_unit = N::abbreviation();
    let base_value = q.value.to_f64().unwrap();
    format!("{}{}", format_value_with_prefix(base_value), base_unit)
}
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
/// An enum defining value constraints for various [`Proptype`]s.
pub enum PropCondition {
    /// Allow only non-empty [`Proptype::String`]s.
    NonEmptyString,
    /// Do not use yet...
    InternalOnly, // DO NOT USE YET (deserialization problems)
    /// This property is readonly. It can only be set during property creation.
    ReadOnly, // can only be set during creation
    /// Restrict integer or float properties to values greater (>) than the given limit.
    GreaterThan(f64),
    /// Restrict integer or float properties to values less (<) than the given limit.
    LessThan(f64),
    /// Restrict integer or float properties to values greater than or equal (>=) the given limit.
    GreaterThanEqual(f64),
    /// Restrict integer or float properties to values less than or equal (<=) the given limit.
    LessThanEqual(f64),
}
#[cfg(test)]
mod test {
    use super::*;
    use assert_matches::assert_matches;
    use uom::si::length::nanometer;
    #[test]
    fn from_string() {
        assert_matches!(Proptype::from(String::new()), Proptype::String(_));
        assert_matches!(Proptype::from(""), Proptype::String(_));
    }
    #[test]
    fn format_value() {
        assert_eq!(format_value_with_prefix(0.0), "   0.000 ");
        assert_eq!(format_value_with_prefix(1.0), "   1.000 ");
        assert_eq!(format_value_with_prefix(999.12345), " 999.123 ");
        assert_eq!(format_value_with_prefix(1001.2345), "   1.001 k");
        assert_eq!(format_value_with_prefix(1234567.12345), "   1.235 M");
        assert_eq!(format_value_with_prefix(1234567890.12345), "   1.235 G");
        assert_eq!(format_value_with_prefix(-1234567890.12345), "  -1.235 G");
        assert_eq!(format_value_with_prefix(0.12345), " 123.450 m");
        assert_eq!(format_value_with_prefix(-0.0000012345), "  -1.235 μ");
        assert_eq!(format_value_with_prefix(f64::INFINITY), "     inf ");
        assert_eq!(format_value_with_prefix(f64::NEG_INFINITY), "    -inf ");
        assert_eq!(format_value_with_prefix(f64::NAN), "     nan ");

        // Note < EPSISLON are coerced to zero...
        assert_eq!(format_value_with_prefix(0.5e-21), "   0.000 ");
        assert_eq!(format_value_with_prefix(0.5e-18), "   0.000 ");

        assert_eq!(format_value_with_prefix(1.0e-15), "   1.000 f");
        assert_eq!(format_value_with_prefix(1.0e-12), "   1.000 p");
        assert_eq!(format_value_with_prefix(1.0e-9), "   1.000 n");

        assert_eq!(format_value_with_prefix(1.0e12), "   1.000 T");
        assert_eq!(format_value_with_prefix(1.0e15), "   1.000 P");
        assert_eq!(format_value_with_prefix(1.0e18), "   1.000 E");
        assert_eq!(format_value_with_prefix(1.0e21), "   1.000 Z");
        assert_eq!(format_value_with_prefix(1.0e24), "   1.000 ?");
    }
    #[test]
    fn format_quantity() {
        assert_eq!(
            super::format_quantity(meter, Length::new::<nanometer>(1053.12345)),
            "   1.053 μm"
        );

        // Note: format_quantity does not (yet) check if unit and dimension are compatible:
        assert_eq!(
            super::format_quantity(joule, Length::new::<nanometer>(1053.12345)),
            "   1.053 μJ"
        );
    }
}
