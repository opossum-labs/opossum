#![warn(missing_docs)]
//! Module for handling properties of optical nodes.
use std::cell::RefCell;

use crate::{
    analyzers::ghostfocus::GhostFocusHistory,
    apertures::ApertureShape,
    core_optics::hit_map::{HitMap, fluence_estimator::FluenceEstimator},
    error::{OpmResult, OpossumError},
    light::{
        Spectrum,
        lightdata::{
            LightData,
            light_data_builder::LightDataBuilder,
            ray_data_source::{CollimatedSrc, ImageSrc, PointSrc, RayDataSource},
        },
    },
    material::Material,
    nodes::{
        Metertype, SpectrometerType, SplittingConfigBuilder, SpotDiagramData, WaveFrontMap,
        WavefrontDecomposition,
        fluence_detector::{Fluence, fluence_data::FluenceData},
        ideal_filter::FilterTypeBuilder,
        ray_propagation_visualizer::RayPositionHistories,
        reflective_grating::LinearDensity,
    },
    refractive_index::RefractiveIndexType,
    reporting::{html_report::HtmlNodeReport, node_report::NodeReport},
    utils::{
        geom_transformation::Isometry,
        unit_format::{get_exponent_for_base_unit_in_e3_steps, get_prefix_for_base_unit},
    },
};
use nalgebra::{Vector2, Vector3};
use num_traits::Float;
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;
use uom::si::{
    Dimension, Quantity, Unit, Units,
    energy::joule,
    f64::{Energy, Length, LinearNumberDensity},
    length::meter,
    radiant_exposure::joule_per_square_centimeter,
};
use uom::{lib::fmt::Debug, si::f64::Angle};
use uuid::Uuid;

static HTML_PROP_SIMPLE: &str = include_str!("../html/prop_simple.html");
static HTML_PROP_IMAGE: &str = include_str!("../html/prop_image.html");
static HTML_PROP_GROUP: &str = include_str!("../html/node_report.html");
static HTML_PROP_DECOMPOSITION: &str = include_str!("../html/prop_decomposition.html");

thread_local! {
    static THREAD_TEMPLATES: RefCell<TinyTemplate<'static>> = RefCell::new({
        let mut tt = TinyTemplate::new();
        tt.add_template("simple", HTML_PROP_SIMPLE)
            .expect("Failed to add simple template (thread-local)");
        tt.add_template("image", HTML_PROP_IMAGE)
            .expect("Failed to add image template (thread-local)");
        tt.add_template("group", HTML_PROP_GROUP)
            .expect("Failed to add group template (thread-local)");
        tt.add_template("decomposition", HTML_PROP_DECOMPOSITION)
            .expect("Failed to add decomposition template (thread-local)");
        tt
    });
}

/// Wrapper for registry assets that can hold either the full inline data
/// or a lightweight UUID reference for serialization.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum AssetRef<T> {
    /// Full in-memory asset struct.
    Inline(T),
    /// Lightweight UUID reference.
    Id(Uuid),
}

impl<T> AssetRef<T> {
    /// Returns a reference to the inline asset if available.
    #[must_use]
    pub const fn as_inline(&self) -> Option<&T> {
        match self {
            Self::Inline(asset) => Some(asset),
            Self::Id(_) => None,
        }
    }

    /// Returns a mutable reference to the inline asset if available.
    #[must_use]
    pub const fn as_inline_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Inline(asset) => Some(asset),
            Self::Id(_) => None,
        }
    }

    /// Extracts the inline asset reference, panicking if it is a pure ID reference.
    ///
    /// # Panics
    ///
    /// As the function name already says, this function panics one tires to unwrap an [`AssetRef::Id`] item.
    #[must_use]
    pub fn unwrap_inline(&self) -> &T {
        match self {
            Self::Inline(asset) => asset,
            Self::Id(id) => panic!("Expected inline asset, found AssetRef::Id({id})"),
        }
    }

    /// Extracts a mutable inline asset reference, panicking if it is a pure ID reference.
    ///
    /// # Panics
    ///
    /// As the function name already says, this function panics one tires to unwrap an [`AssetRef::Id`] item.
    #[must_use]
    pub fn unwrap_inline_mut(&mut self) -> &mut T {
        match self {
            Self::Inline(asset) => asset,
            Self::Id(id) => panic!("Expected inline asset, found AssetRef::Id({id})"),
        }
    }
}

/// Generic conversion from any concrete asset type T into an [`AssetRef::Inline`].
impl<T> From<T> for AssetRef<T> {
    fn from(value: T) -> Self {
        Self::Inline(value)
    }
}

/// Conversion from [`AssetRef<Material>`] into Proptype.
impl From<AssetRef<Material>> for Proptype {
    fn from(value: AssetRef<Material>) -> Self {
        Self::Material(value)
    }
}

// #[non_exhaustive]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    // /// Property for storing a [`FilterType`] of an [`IdealFilter`](crate::nodes::IdealFilter) node.
    // FilterType(FilterType),
    /// Property for storing a [`FilterTypeBuilder`] of an [`IdealFilter`](crate::nodes::IdealFilter) node.
    FilterTypeBuilder(FilterTypeBuilder),
    /// Property for storing a [`SplittingConfig`](crate::nodes::SplittingConfig) of an [`BeamSplitter`](crate::nodes::BeamSplitter) node.
    SplittingConfigBuilder(SplittingConfigBuilder),
    /// Property for storing a [`SpectrometerType`] of a [`Sepctrometer`](crate::nodes::Spectrometer) node.
    SpectrometerType(SpectrometerType),
    /// Property for storing a [`Metertype`] of an [`Energymeter`](crate::nodes::EnergyMeter) node.
    Metertype(Metertype),
    /// An [`Uuid`] for identifying an optical node.
    Uuid(Uuid),
    /// A property for storing an optical [`Aperture`](crate::apertures::Aperture).
    Aperture(ApertureShape),
    /// A property for storing a [`Spectrum`].
    Spectrum(Spectrum),
    /// This property stores the fluence information [`FluenceData`]
    FluenceData(FluenceData),
    /// This property stores the fluence estimator strategy [`FluenceEstimator`]
    FluenceEstimator(FluenceEstimator),
    /// This property stores the wavefront Information [`WaveFrontMap`]
    WaveFrontData(WaveFrontMap),
    /// Polynomial wavefront basis decomposition
    WavefrontDecomposition(WavefrontDecomposition),
    /// This property stores the ray position history of all [`Rays`](crate::light::Rays) during propagation through the optic scenery
    RayPositionHistory(RayPositionHistories),
    /// This property stores the ray position history of all [`Rays`](crate::light::Rays), separated by their bounce level,
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
    /// a curvature
    Curvature(Length),
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
    /// a hit map (position of rays hitting a given surface)
    HitMap(HitMap),
    /// 2D spot diagram data resolved by wavelength
    SpotDiagram(SpotDiagramData),
    /// 2-dimenstional vector
    Vec2(Vector2<f64>),
    /// [`LightData`] build configuration
    LightDataBuilder(LightDataBuilder),
    /// An optical material property
    Material(AssetRef<Material>),
}
impl Proptype {
    /// Generate a html representation of a Proptype.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - underlying html templates could not be compiled
    ///   - a property value could not be converted to html code.
    pub fn to_html(
        &self,
        id: &str,
        property_name: &str,
        report_number: usize,
    ) -> OpmResult<String> {
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
                Self::Spectrum(_)
                | Self::HitMap(_)
                | Self::SpotDiagram(_)
                | Self::RayPositionHistory(_)
                | Self::GhostFocusHistory(_) => template_engine.render(
                    "image",
                    &format!("data_{report_number}/{id}_{property_name}.svg"),
                ),
                Self::WaveFrontData(_) | Self::FluenceData(_) => template_engine.render(
                    "image",
                    &format!("data_{report_number}/{id}_{property_name}.png"),
                ),
                Self::NodeReport(report) => {
                    let html_node_report = HtmlNodeReport::from_node_report(report, report_number);
                    template_engine.render("group", &html_node_report)
                }
                Self::WavefrontDecomposition(decomp) => {
                    render_decomposition_html(decomp, &template_engine)
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
                Self::Aperture(shape) => template_engine.render("simple", &shape.to_string()),
                Self::Material(AssetRef::Inline(mat)) => template_engine
                    .render("simple", &format!("{} (v{})", mat.name(), mat.version())),
                Self::Material(AssetRef::Id(mat_id)) => {
                    template_engine.render("simple", &format!("MaterialRef({mat_id})"))
                }
                _ => Err(tinytemplate::error::Error::GenericError {
                    msg: "proptype not supported".into(),
                }),
            };
            string_value.map_err(|e| OpossumError::Other(format!("Template rendering error: {e}")))
        })
    }
    /// Returns the raw scalar value as an `f64` if this variant represents a numeric scalar.
    /// For dimensional `uom` quantities, the value is returned in its fundamental SI base unit.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(v) | Self::WfLambda(v, _) => Some(*v),
            Self::I32(v) => Some(f64::from(*v)),
            Self::Length(v) | Self::Curvature(v) => Some(v.value),
            Self::Angle(v) => Some(v.value),
            Self::Energy(v) => Some(v.value),
            Self::Fluence(v) => Some(v.value),
            Self::LinearDensity(v) => Some(v.value),
            _ => None,
        }
    }
    /// Sets the scalar value directly if this variant represents a numeric scalar.
    /// Dimensional quantities are updated in their fundamental SI base unit.
    ///
    /// # Errors
    /// Returns an [`OpossumError::Properties`] if the variant does not support scalar numeric assignment.
    #[allow(clippy::cast_possible_truncation)]
    pub fn set_f64(&mut self, value: f64) -> OpmResult<()> {
        match self {
            Self::F64(v) | Self::WfLambda(v, _) => {
                *v = value;
                Ok(())
            }
            Self::I32(v) => {
                *v = value.round() as i32;
                Ok(())
            }
            Self::Length(v) | Self::Curvature(v) => {
                v.value = value;
                Ok(())
            }
            Self::Angle(v) => {
                v.value = value;
                Ok(())
            }
            Self::Energy(v) => {
                v.value = value;
                Ok(())
            }
            Self::Fluence(v) => {
                v.value = value;
                Ok(())
            }
            Self::LinearDensity(v) => {
                v.value = value;
                Ok(())
            }
            _ => Err(OpossumError::Properties(
                "Property variant cannot be set from a scalar f64 value".into(),
            )),
        }
    }
}
/// Helper function to build the template context and render the decomposition table to HTML.
fn render_decomposition_html(
    decomp_data: &WavefrontDecomposition,
    template_engine: &TinyTemplate<'_>,
) -> Result<String, tinytemplate::error::Error> {
    const NOISE_THRESHOLD_LAMBDA: f64 = 1e-6;
    let wvl = decomp_data.wavelength;

    let residual_rms_val = if decomp_data.decomposition.residual_rms.abs() < NOISE_THRESHOLD_LAMBDA
    {
        0.0
    } else {
        decomp_data.decomposition.residual_rms
    };

    let mut modes = Vec::new();
    for mode in &decomp_data.decomposition.modes {
        // Skip global phase shift (piston)
        if mode.is_piston {
            continue;
        }

        // Cut off sub-micro-lambda numerical noise
        let coeff = if mode.coefficient.abs() < NOISE_THRESHOLD_LAMBDA {
            0.0
        } else {
            mode.coefficient
        };

        let coeff_lambda_str = format!("{}λ", format_value_with_prefix(coeff));
        let physical_val = coeff * wvl.value; // Value in fundamental base unit (meter)
        let coeff_phys_str = format_quantity(meter, Length::new::<meter>(physical_val));

        modes.push(HtmlDecompMode {
            label: mode.label.clone(),
            coeff_lambda: coeff_lambda_str,
            coeff_physical: coeff_phys_str,
        });
    }

    let metadata = decomp_data
        .decomposition
        .metadata
        .iter()
        .map(|(k, v)| HtmlDecompMeta {
            key: k.clone(),
            val: v.clone(),
        })
        .collect();

    let ctx = HtmlDecompContext {
        basis_name: decomp_data.decomposition.basis_name.clone(),
        residual_rms: format!("{}λ", format_value_with_prefix(residual_rms_val)),
        metadata,
        modes,
    };

    template_engine.render("decomposition", &ctx)
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
impl From<Option<Length>> for Proptype {
    fn from(value: Option<Length>) -> Self {
        Self::LengthOption(value)
    }
}
impl From<Isometry> for Proptype {
    fn from(value: Isometry) -> Self {
        Self::Isometry(Some(value))
    }
}
impl From<LinearNumberDensity> for Proptype {
    fn from(val: LinearNumberDensity) -> Self {
        Self::LinearDensity(val)
    }
}
impl From<FluenceEstimator> for Proptype {
    fn from(val: FluenceEstimator) -> Self {
        Self::FluenceEstimator(val)
    }
}
impl From<SpotDiagramData> for Proptype {
    fn from(value: SpotDiagramData) -> Self {
        Self::SpotDiagram(value)
    }
}
impl From<Vector2<f64>> for Proptype {
    fn from(value: Vector2<f64>) -> Self {
        Self::Vec2(value)
    }
}
impl From<Vector3<f64>> for Proptype {
    fn from(value: Vector3<f64>) -> Self {
        Self::Vec3(value)
    }
}
impl From<CollimatedSrc> for Proptype {
    fn from(val: CollimatedSrc) -> Self {
        Self::LightDataBuilder(LightDataBuilder::Geometric(RayDataSource::Collimated(val)))
    }
}
impl From<PointSrc> for Proptype {
    fn from(val: PointSrc) -> Self {
        Self::LightDataBuilder(LightDataBuilder::Geometric(RayDataSource::PointSrc(val)))
    }
}
impl From<ImageSrc> for Proptype {
    fn from(val: ImageSrc) -> Self {
        Self::LightDataBuilder(LightDataBuilder::Geometric(RayDataSource::Image(val)))
    }
}
impl From<LightDataBuilder> for Proptype {
    fn from(val: LightDataBuilder) -> Self {
        Self::LightDataBuilder(val)
    }
}
impl From<FilterTypeBuilder> for Proptype {
    fn from(val: FilterTypeBuilder) -> Self {
        Self::FilterTypeBuilder(val)
    }
}

impl From<Material> for Proptype {
    fn from(value: Material) -> Self {
        Self::Material(AssetRef::Inline(value))
    }
}

impl From<WavefrontDecomposition> for Proptype {
    fn from(value: WavefrontDecomposition) -> Self {
        Self::WavefrontDecomposition(value)
    }
}
#[derive(Serialize)]
struct HtmlDecompMeta {
    key: String,
    val: String,
}

#[derive(Serialize)]
struct HtmlDecompMode {
    label: String,
    coeff_lambda: String,
    coeff_physical: String,
}

#[derive(Serialize)]
struct HtmlDecompContext {
    basis_name: String,
    residual_rms: String,
    metadata: Vec<HtmlDecompMeta>,
    modes: Vec<HtmlDecompMode>,
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
    use std::f64::consts;

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
    fn to_html() -> OpmResult<()> {
        assert_eq!(
            Proptype::String("Test".into()).to_html("id", "property_name", 0)?,
            "Test".to_string()
        );
        assert_eq!(
            Proptype::I32(-14).to_html("id", "property_name", 0)?,
            "-14".to_string()
        );
        assert_eq!(
            Proptype::F64(-3.1415926537).to_html("id", "property_name", 0)?,
            "-3.141593".to_string()
        );
        assert_eq!(
            Proptype::Bool(true).to_html("id", "property_name", 0)?,
            "true".to_string()
        );
        assert_eq!(
            Proptype::SpectrometerType(SpectrometerType::HR2000).to_html(
                "id",
                "property_name",
                0
            )?,
            "Ocean Optics HR2000".to_string()
        );
        // An embedded material is named by its header, a referenced one only by its id.
        assert_eq!(
            Proptype::from(Material::default()).to_html("id", "property_name", 0)?,
            "Default Glass (v0)".to_string()
        );
        let material_id = Uuid::nil();
        assert_eq!(
            Proptype::Material(AssetRef::<Material>::Id(material_id)).to_html(
                "id",
                "property_name",
                0
            )?,
            format!("MaterialRef({material_id})")
        );
        assert_eq!(
            Proptype::WaveFrontData(WaveFrontMap::default())
                .to_html("id", "property_name",0)
                ?,
            "<img src=\"data_0/id_property_name.png\" class=\"img-fluid\" style=\"max-height: 500pt;\" alt=\"measurement data\"/>".to_string()
        );
        assert_eq!(
            Proptype::NodeReport(NodeReport::new("test1", "test2", "test3", Properties::default()))
                .to_html("id", "property_name",0)
                ?,
            "<div class=\"accordion-item\">\n  <h5 class=\"accordion-header\">\n    <button class=\"accordion-button collapsed\" type=\"button\" data-bs-toggle=\"collapse\" data-bs-target=\"#test3\">\n      <span class=\"h5 me-2\">test2</span><small class=\"muted\">test1</small>\n    </button>\n  </h5>\n  <div id=\"test3\" class=\"accordion-collapse collapse \">\n    <div class=\"accordion-body\">\n      \n      <table class=\"table table-sm table-bordered\">\n        <tbody>\n          \n        </tbody>\n      </table>\n    </div>\n  </div>\n</div>".to_string()
        );
        assert_eq!(
            Proptype::Fluence(J_per_m2!(1.234567)).to_html("id", "property_name", 0)?,
            " 123.457 μJ/cm²".to_string()
        );
        assert_eq!(
            Proptype::WfLambda(0.123456, nanometer!(1054.0)).to_html("id", "property_name", 0)?,
            " 123.456 mλ, (λ =    1.054 μm)".to_string()
        );
        assert_eq!(
            Proptype::Length(meter!(0.12345678)).to_html("id", "property_name", 0)?,
            " 123.457 mm".to_string()
        );
        assert_eq!(
            Proptype::Energy(joule!(0.12345678)).to_html("id", "property_name", 0)?,
            " 123.457 mJ".to_string()
        );
        Ok(())
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
    #[test]
    fn asset_ref_inline_and_id() {
        let inline_ref: AssetRef<i32> = AssetRef::from(42);
        assert_eq!(inline_ref.as_inline(), Some(&42));
        assert_eq!(*inline_ref.unwrap_inline(), 42);

        let test_uuid = Uuid::new_v4();
        let id_ref: AssetRef<i32> = AssetRef::Id(test_uuid);
        assert_eq!(id_ref.as_inline(), None);
    }

    #[test]
    #[should_panic(expected = "Expected inline asset, found AssetRef::Id")]
    fn asset_ref_unwrap_panic() {
        let id_ref: AssetRef<i32> = AssetRef::Id(Uuid::new_v4());
        let _ = id_ref.unwrap_inline();
    }
    #[test]
    fn scalar_f64_access_and_mutation() -> OpmResult<()> {
        use uom::si::length::millimeter;

        // F64 variant
        let mut prop_f64 = Proptype::F64(consts::PI);
        assert_eq!(prop_f64.as_f64(), Some(consts::PI));
        prop_f64.set_f64(consts::E)?;
        assert_eq!(prop_f64.as_f64(), Some(consts::E));

        // I32 variant
        let mut prop_i32 = Proptype::I32(10);
        assert_eq!(prop_i32.as_f64(), Some(10.0));
        prop_i32.set_f64(25.4)?;
        assert_eq!(prop_i32, Proptype::I32(25));

        // UOM Length variant (operates strictly on SI base unit meter)
        let mut prop_len = Proptype::Length(Length::new::<millimeter>(50.0));
        assert!((prop_len.as_f64().unwrap() - 0.05).abs() < 1e-12);
        prop_len.set_f64(0.125)?;
        assert_eq!(prop_len, Proptype::Length(Length::new::<meter>(0.125)));

        // Non-scalar variant must fail
        let mut prop_bool = Proptype::Bool(true);
        assert_eq!(prop_bool.as_f64(), None);
        assert!(prop_bool.set_f64(1.0).is_err());
        Ok(())
    }
}
