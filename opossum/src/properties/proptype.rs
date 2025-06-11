#![warn(missing_docs)]
//! Module for handling properties of optical nodes.
use std::cell::RefCell;

use crate::{
    analyzers::ghostfocus::GhostFocusHistory,
    aperture::Aperture,
    error::{OpmResult, OpossumError},
    lightdata::{LightData, light_data_builder::LightDataBuilder},
    nodes::{
        FilterType, Metertype, Spectrometer, SpectrometerType, SpotDiagram, WaveFrontData,
        fluence_detector::{Fluence, fluence_data::FluenceData},
        ray_propagation_visualizer::RayPositionHistories,
        reflective_grating::LinearDensity,
    },
    optic_ports::OpticPorts,
    ray::SplittingConfig,
    refractive_index::RefractiveIndexType,
    reporting::{html_report::HtmlNodeReport, node_report::NodeReport},
    surface::hit_map::{HitMap, fluence_estimator::FluenceEstimator},
    utils::{
        geom_transformation::Isometry,
        unit_format::{get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit},
    },
};
use nalgebra::{Vector2, Vector3};
use num::Float;
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;
use uom::si::{
    Dimension, Quantity, Unit, Units,
    energy::joule,
    f64::{Energy, Length},
    length::meter,
    radiant_exposure::joule_per_square_centimeter,
};
use uom::{lib::fmt::Debug, si::f64::Angle};
use uuid::Uuid;

static HTML_PROP_SIMPLE: &str = include_str!("../html/prop_simple.html");
static HTML_PROP_IMAGE: &str = include_str!("../html/prop_image.html");
static HTML_PROP_GROUP: &str = include_str!("../html/node_report.html");

thread_local! {
    static THREAD_TEMPLATES: RefCell<TinyTemplate<'static>> = RefCell::new({
        let mut tt = TinyTemplate::new();
        tt.add_template("simple", HTML_PROP_SIMPLE)
            .expect("Failed to add simple template (thread-local)");
        tt.add_template("image", HTML_PROP_IMAGE)
            .expect("Failed to add image template (thread-local)");
        tt.add_template("group", HTML_PROP_GROUP)
            .expect("Failed to add group template (thread-local)");
        tt
    });
}

#[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone)]
/// The type of the [`Property`](crate::properties::Property).
pub enum Proptype {
    /// A string property
    String(String),
    /// An integer property
    I32(i32),
    /// A float property
    F64(f64),
    /// A boolean property
    Bool(bool),
    /// An optional [`LightData`] property
    LightData(Option<LightData>),
    /// Property for storing a [`FilterType`] of an [`IdealFilter`](crate::nodes::IdealFilter) node.
    FilterType(FilterType),
    /// Property for storing a [`SplittingConfig`] of an [`BeamSplitter`](crate::nodes::BeamSplitter) node.
    SplitterType(SplittingConfig),
    /// Property for storing a [`SpectrometerType`] of a [`Sepctrometer`](crate::nodes::Spectrometer) node.
    SpectrometerType(SpectrometerType),
    /// Property for storing a [`Metertype`] of an [`Energymeter`](crate::nodes::EnergyMeter) node.
    Metertype(Metertype),
    /// An [`Uuid`] for identifying an optical node.
    Uuid(Uuid),
    /// A property for storing an optical [`Aperture`].
    Aperture(Aperture),
    /// A property for storing a [`Spectrum`](crate::spectrum::Spectrum).
    Spectrometer(Spectrometer),
    /// This property stores optical [`Rays`](crate::rays::Rays)
    SpotDiagram(SpotDiagram),
    /// This property stores the fluence information [`FluenceData`]
    FluenceData(FluenceData),
    /// This property stores the fluence estimator strategy [`FluenceEstimator`]
    FluenceEstimator(FluenceEstimator),
    /// This property stores the wavefront Information [`WaveFrontData`]
    WaveFrontData(WaveFrontData),
    /// This property stores the ray position history of all [`Rays`](crate::rays::Rays) during propagation through the optic scenery
    RayPositionHistory(RayPositionHistories),
    /// This property stores the ray position history of all [`Rays`](crate::rays::Rays), separated by their bounce level,
    /// during propagation through the optic scenery
    GhostFocusHistory(GhostFocusHistory),
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
    /// an optional length parameter. used, e.g., for the alignment wavelength of the source
    LengthOption(Option<Length>),
    /// an energy value
    Energy(Energy),
    /// a (2D) geometric angle (e.g. component tilt)
    Angle(Angle),
    /// an optical refractive index model
    RefractiveIndex(RefractiveIndexType),
    /// a (node) location / orientation
    Isometry(Option<Isometry>),
    /// Three dimensional Vector
    Vec3(Vector3<f64>),
    /// a hit map (position fo rays hitting a given surface)
    HitMap(HitMap),
    /// 2-dimenstional vector
    Vec2(Vector2<f64>),
    /// [`LightData`] build configuration
    LightDataBuilder(Option<LightDataBuilder>),
}
impl Proptype {
    /// Generate a html representation of a Proptype.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying html templates could not be compiled
    ///   - a property value could not be converted to html code.
    pub fn to_html(&self, id: &str, property_name: &str) -> OpmResult<String> {
        THREAD_TEMPLATES.with(|template_refcell| {
            let template_engine = template_refcell.borrow();
            let string_value = match self {
                Self::String(value) => template_engine.render("simple", value),
                Self::I32(value) => template_engine.render("simple", &format!("{value}")),
                Self::F64(value) => template_engine.render("simple", &format!("{value:.6}")),
                Self::Bool(value) => template_engine.render("simple", &format!("{value}")),
                Self::SpectrometerType(value) => {
                    template_engine.render("simple", &value.to_string())
                }
                Self::Metertype(value) => template_engine.render("simple", &value.to_string()),
                Self::Spectrometer(_)
                | Self::SpotDiagram(_)
                | Self::HitMap(_)
                | Self::RayPositionHistory(_)
                | Self::GhostFocusHistory(_) => {
                    template_engine.render("image", &format!("data/{id}_{property_name}.svg"))
                }
                Self::WaveFrontData(_) | Self::FluenceData(_) => {
                    template_engine.render("image", &format!("data/{id}_{property_name}.png"))
                }
                Self::NodeReport(report) => {
                    let html_node_report = HtmlNodeReport {
                        node_name: report.name().into(),
                        node_type: report.node_type().into(),
                        props: report.properties().html_props(&format!(
                            "{id}_{}_{}",
                            report.name(),
                            report.uuid()
                        )),
                        uuid: report.uuid().to_string(),
                        show_item: report.show_item(),
                    };
                    template_engine.render("group", &html_node_report)
                }
                Self::Fluence(value) => template_engine.render(
                    "simple",
                    &format!(
                        "{}{}",
                        format_value_with_prefix(value.get::<joule_per_square_centimeter>()),
                        joule_per_square_centimeter::abbreviation()
                    ),
                ),
                Self::WfLambda(value, wvl) => template_engine.render(
                    "simple",
                    &format!(
                        "{}λ, (λ = {})",
                        format_value_with_prefix(*value,),
                        format_quantity(meter, *wvl)
                    ),
                ),
                Self::Length(value) => {
                    template_engine.render("simple", &format_quantity(meter, *value))
                }
                Self::Energy(value) => {
                    template_engine.render("simple", &format_quantity(joule, *value))
                }
                _ => Err(tinytemplate::error::Error::GenericError {
                    msg: "proptype not supported".into(),
                }),
            };
            string_value.map_err(|e| OpossumError::Other(format!("Template rendering error: {e}")))
        })
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
impl From<Vector2<f64>> for Proptype {
    fn from(value: Vector2<f64>) -> Self {
        Self::Vec2(value)
    }
}
/// Generate a string suffix for an ordinal number
#[must_use]
pub fn count_str(i: usize) -> String {
    let mod_i = i % 10;
    let suf = match mod_i {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{i}{suf}")
}

/// Generate a value string with a SI prefix.
///
/// Helper function to format a value with a SI prefix.
#[must_use]
pub fn format_value_with_prefix(value: f64) -> String {
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
    let prefix = get_prefix_for_base_unit(value);
    let exponent = get_exponent_for_base_unit_in_e3_steps(value);

    format!("{:8.3} {prefix}", value / f64::powi(10.0, exponent))
}
/// Formats a uom quantity
///
/// # Panics
/// This function panics if the conversion from the quantity value to f64 fails.
pub fn format_quantity<D, U, V, N>(_: N, q: Quantity<D, U, V>) -> String
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
#[cfg(test)]
mod test {
    use super::*;
    use crate::{J_per_m2, joule, meter, nanometer, properties::Properties};
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
    #[test]
    fn to_html() {
        assert_eq!(
            Proptype::String("Test".into())
                .to_html("id", "property_name")
                .unwrap(),
            "Test".to_string()
        );
        assert_eq!(
            Proptype::I32(-14).to_html("id", "property_name").unwrap(),
            "-14".to_string()
        );
        assert_eq!(
            Proptype::F64(-3.1415926537)
                .to_html("id", "property_name")
                .unwrap(),
            "-3.141593".to_string()
        );
        assert_eq!(
            Proptype::Bool(true).to_html("id", "property_name").unwrap(),
            "true".to_string()
        );
        assert_eq!(
            Proptype::SpectrometerType(SpectrometerType::HR2000)
                .to_html("id", "property_name")
                .unwrap(),
            "Ocean Optics HR2000".to_string()
        );
        assert_eq!(
            Proptype::SpotDiagram(SpotDiagram::default())
                .to_html("id", "property_name")
                .unwrap(),
            "<img src=\"data/id_property_name.svg\" class=\"img-fluid\" style=\"max-height: 500pt;\" alt=\"measurement data\"/>".to_string()
        );
        assert_eq!(
            Proptype::WaveFrontData(WaveFrontData::default())
                .to_html("id", "property_name")
                .unwrap(),
            "<img src=\"data/id_property_name.png\" class=\"img-fluid\" style=\"max-height: 500pt;\" alt=\"measurement data\"/>".to_string()
        );
        assert_eq!(
            Proptype::NodeReport(NodeReport::new("test1", "test2", "test3", Properties::default()))
                .to_html("id", "property_name")
                .unwrap(),
            "<div class=\"accordion-item\">\n  <h5 class=\"accordion-header\">\n    <button class=\"accordion-button\" type=\"button\" data-bs-toggle=\"collapse\" data-bs-target=\"#test3\">\n      <span class=\"h5 me-2\">test2</span><small class=\"muted\">test1</small>\n    </button>\n  </h5>\n  <div id=\"test3\" class=\"accordion-collapse collapse \">\n    <div class=\"accordion-body\">\n      <table class=\"table table-sm table-bordered\">\n        <tbody>\n          \n        </tbody>\n      </table>\n    </div>\n  </div>\n</div>\n".to_string()
        );
        assert_eq!(
            Proptype::Fluence(J_per_m2!(1.234567))
                .to_html("id", "property_name")
                .unwrap(),
            " 123.457 μJ/cm²".to_string()
        );
        assert_eq!(
            Proptype::WfLambda(0.123456, nanometer!(1054.0))
                .to_html("id", "property_name")
                .unwrap(),
            " 123.456 mλ, (λ =    1.054 μm)".to_string()
        );
        assert_eq!(
            Proptype::Length(meter!(0.12345678))
                .to_html("id", "property_name")
                .unwrap(),
            " 123.457 mm".to_string()
        );
        assert_eq!(
            Proptype::Energy(joule!(0.12345678))
                .to_html("id", "property_name")
                .unwrap(),
            " 123.457 mJ".to_string()
        );
    }
    #[test]
    fn test_count_str() {
        assert_eq!(count_str(0), "0th");
        assert_eq!(count_str(1), "1st");
        assert_eq!(count_str(2), "2nd");
        assert_eq!(count_str(3), "3rd");
        assert_eq!(count_str(4), "4th");
        assert_eq!(count_str(20), "20th");
        assert_eq!(count_str(21), "21st");
        assert_eq!(count_str(22), "22nd");
        assert_eq!(count_str(23), "23rd");
        assert_eq!(count_str(24), "24th");
    }
}
