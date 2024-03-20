use crate::{
    aperture::Aperture,
    error::OpmResult,
    lightdata::LightData,
    nodes::{
        ray_propagation_visualizer::RayPositionHistories, FilterType, FluenceData, Metertype,
        PortMap, Spectrometer, SpectrometerType, SpotDiagram, WaveFrontData,
    },
    optic_graph::OpticGraph,
    optic_ports::OpticPorts,
    ray::SplittingConfig,
    refractive_index::RefractiveIndexType,
    reporter::{NodeReport, PdfReportable},
    utils::EnumProxy,
};
use genpdf::style;
use num::Float;
use serde_derive::{Deserialize, Serialize};
use uom::lib::fmt::Debug;
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::meter,
    Dimension, Quantity, Unit, Units,
};
use uuid::Uuid;

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
/// The type of the [`Property`].
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
    /// Property for storing the external port mapping ([`PortMap`]) of a [`Group`](crate::nodes::NodeGroup) node.
    GroupPortMap(PortMap),
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
    /// Fluence in Units of J/cm²
    Fluence(f64),
    /// Unit of Wavelength
    WfLambda(f64, Length),
    /// a geometrical length
    Length(Length),
    /// an energy value
    Energy(Energy),
    /// a optical refractive index model
    RefractiveIndex(EnumProxy<RefractiveIndexType>),
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
impl PdfReportable for Proptype {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        let mut l = genpdf::elements::LinearLayout::vertical();
        match self {
            Self::String(value) => l.push(genpdf::elements::Paragraph::new(value)),
            Self::I32(value) => l.push(genpdf::elements::Paragraph::new(format!("{value}"))),
            Self::F64(value) => l.push(genpdf::elements::Paragraph::new(format!("{value:.6}"))),
            Self::Bool(value) => l.push(genpdf::elements::Paragraph::new(value.to_string())),
            Self::FilterType(value) => l.push(value.value.pdf_report()?),
            Self::SpectrometerType(value) => l.push(value.pdf_report()?),
            Self::Metertype(value) => l.push(value.pdf_report()?),
            Self::Spectrometer(value) => l.push(value.pdf_report()?),
            Self::SpotDiagram(value) => l.push(value.pdf_report()?),
            Self::WaveFrontStats(value) => l.push(value.pdf_report()?),
            Self::FluenceDetector(value) => l.push(value.pdf_report()?),
            Self::NodeReport(value) => l.push(value.properties().pdf_report()?),
            Self::Fluence(value) => l.push(genpdf::elements::Paragraph::new(format!(
                "{}J/cm²",
                format_value_with_prefix(*value)
            ))),
            Self::WfLambda(value, wvl) => l.push(genpdf::elements::Paragraph::new(format!(
                "{}λ, (λ = {})",
                format_value_with_prefix(*value,),
                format_quantity(meter, *wvl)
            ))),
            Self::Length(value) => l.push(genpdf::elements::Paragraph::new(format_quantity(
                meter, *value,
            ))),
            Self::Energy(value) => l.push(genpdf::elements::Paragraph::new(format_quantity(
                joule, *value,
            ))),
            Self::RayPositionHistory(value) => l.push(value.pdf_report()?),

            _ => l.push(
                genpdf::elements::Paragraph::default()
                    .styled_string("unknown property type", style::Effect::Italic),
            ),
        }
        Ok(l)
    }
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
    }
}
