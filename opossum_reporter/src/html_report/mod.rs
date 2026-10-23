//! Module for generating html reports from analysis results.

use log::{info, warn};
use serde::Serialize;
use std::{cell::RefCell, fs, path::Path};
use tinytemplate::TinyTemplate;
use uom::si::{
    Unit, energy::joule, f64::Length, length::meter, radiant_exposure::joule_per_square_centimeter,
};

use opossum_core::{
    core_optics::node_attr::HasNodeAttr,
    nodes::WavefrontDecomposition,
    properties::{
        Properties, Proptype,
        proptype::{AssetRef, format_quantity, format_value_with_prefix},
    },
    reporting::{
        analysis_report::AnalysisReport,
        node_report::NodeReport,
        report_note::{ReportLevel, ReportNote},
    },
    utils::file_utils::sanitize_filename,
};

use crate::{
    error::{ReporterError, ReporterResult},
    plotting::export_node_report_plots,
};

static HTML_PROP_SIMPLE: &str = include_str!("../html/prop_simple.html");
static HTML_PROP_IMAGE: &str = include_str!("../html/prop_image.html");
static HTML_PROP_GROUP: &str = include_str!("../html/node_report.html");
static HTML_PROP_DECOMPOSITION: &str = include_str!("../html/prop_decomposition.html");
static HTML_REPORT: &str = include_str!("../html/html_report.html");
static HTML_NODE_REPORT: &str = include_str!("../html/node_report.html");

thread_local! {
    static THREAD_TEMPLATES: RefCell<TinyTemplate<'static>> = RefCell::new({
        let mut tt = TinyTemplate::new();
        tt.add_template("simple", HTML_PROP_SIMPLE)
            .expect("Failed to add simple template");
        tt.add_template("image", HTML_PROP_IMAGE)
            .expect("Failed to add image template");
        tt.add_template("group", HTML_PROP_GROUP)
            .expect("Failed to add group template");
        tt.add_template("decomposition", HTML_PROP_DECOMPOSITION)
            .expect("Failed to add decomposition template");
        tt
    });
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
        if mode.is_piston {
            continue;
        }

        let coeff = if mode.coefficient.abs() < NOISE_THRESHOLD_LAMBDA {
            0.0
        } else {
            mode.coefficient
        };

        let coeff_lambda_str = format!("{}λ", format_value_with_prefix(coeff));
        let physical_val = coeff * wvl.value;
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

/// Convert a [`Proptype`] into an HTML string representation.
///
/// # Errors
/// Returns an error if template rendering fails or the property type cannot be converted to HTML.
pub fn proptype_to_html(
    prop: &Proptype,
    id: &str,
    property_name: &str,
    report_number: usize,
) -> ReporterResult<String> {
    THREAD_TEMPLATES.with(|template_refcell| {
        let template_engine = template_refcell.borrow();
        let string_value = match prop {
            Proptype::String(value) => template_engine.render("simple", value),
            Proptype::I32(value) => template_engine.render("simple", &format!("{value}")),
            Proptype::F64(value) => template_engine.render("simple", &format!("{value:.6}")),
            Proptype::Bool(value) => template_engine.render("simple", &format!("{value}")),
            Proptype::SpectrometerType(value) => {
                template_engine.render("simple", &value.to_string())
            }
            Proptype::Metertype(value) => template_engine.render("simple", &value.to_string()),
            Proptype::Spectrum(_)
            | Proptype::HitMap(_)
            | Proptype::SpotDiagram(_)
            | Proptype::RayPositionHistory(_)
            | Proptype::GhostFocusHistory(_) => template_engine.render(
                "image",
                &format!("data_{report_number}/{id}_{property_name}.svg"),
            ),
            Proptype::WaveFrontData(_) | Proptype::FluenceData(_) => template_engine.render(
                "image",
                &format!("data_{report_number}/{id}_{property_name}.png"),
            ),
            Proptype::NodeReport(report) => {
                let html_node_report = HtmlNodeReport::from_node_report(report, report_number);
                template_engine.render("group", &html_node_report)
            }
            Proptype::WavefrontDecomposition(decomp) => {
                render_decomposition_html(decomp, &template_engine)
            }
            Proptype::Fluence(value) => template_engine.render(
                "simple",
                &format!(
                    "{}{}",
                    format_value_with_prefix(value.get::<joule_per_square_centimeter>()),
                    joule_per_square_centimeter::abbreviation()
                ),
            ),
            Proptype::WfLambda(value, wvl) => template_engine.render(
                "simple",
                &format!(
                    "{}λ, (λ = {})",
                    format_value_with_prefix(*value),
                    format_quantity(meter, *wvl)
                ),
            ),
            Proptype::Length(value) => {
                template_engine.render("simple", &format_quantity(meter, *value))
            }
            Proptype::Energy(value) => {
                template_engine.render("simple", &format_quantity(joule, *value))
            }
            Proptype::OpticShape(optic_shape) => {
                template_engine.render("simple", &optic_shape.to_string())
            }
            Proptype::Material(AssetRef::Inline(mat)) => {
                template_engine.render("simple", &format!("{} (v{})", mat.name(), mat.version()))
            }
            Proptype::Material(AssetRef::Id(mat_id)) => {
                template_engine.render("simple", &format!("MaterialRef({mat_id})"))
            }
            _ => Err(tinytemplate::error::Error::GenericError {
                msg: "proptype not supported for html rendering".into(),
            }),
        };
        string_value.map_err(|e| ReporterError::Html(format!("Template rendering error: {e}")))
    })
}

/// Convert a node's [`Properties`] into a list of [`HtmlProperty`].
#[must_use]
pub fn format_html_props(
    properties: &Properties,
    id: &str,
    report_number: usize,
) -> Vec<HtmlProperty> {
    let mut html_props: Vec<HtmlProperty> = Vec::new();
    for (name, prop) in properties {
        if let Ok(html_prop_value) = proptype_to_html(prop.prop(), id, name, report_number) {
            html_props.push(HtmlProperty {
                name: name.clone(),
                description: prop.description().to_string(),
                prop_value: html_prop_value,
            });
        } else {
            warn!("Property '{name}' could not be converted to HTML. Skipping.");
        }
    }
    html_props
}

#[derive(Serialize)]
pub struct HtmlReportNote {
    pub message: String,
    pub css_class: String,
}

impl From<ReportNote> for HtmlReportNote {
    fn from(note: ReportNote) -> Self {
        let css_class = match note.level {
            ReportLevel::Info => "alert-info",
            ReportLevel::Warning => "alert-warning",
            ReportLevel::Error => "alert-danger",
        };
        Self {
            message: note.message,
            css_class: css_class.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct HtmlReport {
    opossum_version: String,
    analysis_timestamp: String,
    analysis_type: String,
    description: String,
    pub analyzer_name: Option<String>,
    node_reports: Vec<HtmlNodeReport>,
    notes: Vec<HtmlReportNote>,
}

impl HtmlReport {
    #[must_use]
    pub const fn new(
        opossum_version: String,
        analysis_timestamp: String,
        analysis_type: String,
        description: String,
        analyzer_name: Option<String>,
        node_reports: Vec<HtmlNodeReport>,
        notes: Vec<HtmlReportNote>,
    ) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            analysis_type,
            description,
            analyzer_name,
            node_reports,
            notes,
        }
    }

    /// Creates a new [`HtmlReport`] from an [`AnalysisReport`].
    ///
    /// # Errors
    /// Returns an error if the report does not contain scenery.
    pub fn from_analysis_report(
        report: &AnalysisReport,
        report_number: usize,
    ) -> ReporterResult<Self> {
        let Some(scenery) = report.scenery() else {
            return Err(ReporterError::InvalidReport("no scenery found".into()));
        };
        let html_node_reports: Vec<HtmlNodeReport> = report
            .node_reports()
            .iter()
            .map(|node_report| HtmlNodeReport::from_node_report(node_report, report_number))
            .collect();

        Ok(Self::new(
            report.opossum_version().to_string(),
            report
                .analysis_timestamp()
                .format("%Y/%m/%d %H:%M")
                .to_string(),
            report.analysis_type().to_string(),
            scenery.node_attr().name().to_string(),
            report.analyzer_name().map(ToString::to_string),
            html_node_reports,
            report
                .notes()
                .iter()
                .map(|n| HtmlReportNote::from(n.clone()))
                .collect(),
        ))
    }

    /// Generate a complete HTML report, including the main HTML file
    /// and all associated data files.
    ///
    /// # Errors
    /// Returns an error if data directory creation, plot export, or HTML writing fails.
    pub fn generate_report_files(
        &self,
        report_path: &Path,
        analysis_report: &AnalysisReport,
        report_number: usize,
    ) -> ReporterResult<()> {
        // 1. Export associated data first
        let data_dir = report_path.join(format!("data_{report_number}"));
        fs::create_dir_all(&data_dir)?;
        for node_report in analysis_report.node_reports() {
            export_node_report_plots(node_report, &data_dir)?;
        }

        // 2. Render and write the main HTML file
        let mut tt = TinyTemplate::new();
        tt.add_template("report", HTML_REPORT).map_err(|e| {
            ReporterError::Html(format!("Error adding html `report` template: {e}"))
        })?;
        tt.add_template("node_report", HTML_NODE_REPORT)
            .map_err(|e| {
                ReporterError::Html(format!("Error adding html `node_report` template: {e}"))
            })?;
        let rendered = tt.render("report", &self).map_err(|e| {
            ReporterError::Html(format!("Error rendering html `report` template: {e}"))
        })?;
        let html_path = report_path.join(format!("report_{report_number}.html"));
        fs::write(&html_path, rendered)?;
        info!("Write html report to {}", html_path.display());
        Ok(())
    }
}

/// Structure for storing a node report during html conversion.
#[derive(Serialize)]
pub struct HtmlNodeReport {
    /// node name
    pub node_name: String,
    /// node type
    pub node_type: String,
    /// properties of the node
    pub props: Vec<HtmlProperty>,
    /// uuid of the node (needed for constructing filenames)
    pub uuid: String,
    /// show or hide item in report by default
    pub show_item: bool,
    /// notes regarding the node
    pub notes: Vec<HtmlReportNote>,
}

impl HtmlNodeReport {
    /// Create this [`HtmlNodeReport`] from a [`NodeReport`].
    #[must_use]
    pub fn from_node_report(node_report: &NodeReport, report_number: usize) -> Self {
        Self {
            node_name: sanitize_filename(node_report.name()),
            node_type: node_report.node_type().to_string(),
            props: format_html_props(node_report.properties(), node_report.uuid(), report_number),
            uuid: node_report.uuid().to_string(),
            show_item: node_report.show_item(),
            notes: node_report
                .notes()
                .iter()
                .map(|n| HtmlReportNote::from(n.clone()))
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct HtmlProperty {
    pub name: String,
    pub description: String,
    pub prop_value: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::{
        J_per_m2,
        error::OpmResult,
        joule,
        material::Material,
        meter, nanometer,
        nodes::{SpectrometerType, WaveFrontMap},
    };
    use uuid::Uuid;

    #[test]
    fn from_node_report() -> OpmResult<()> {
        let mut properties = Properties::default();
        properties.create("test1", "desc1", 1.0.into())?;
        properties.create("test2", "desc2", "test".into())?;
        let report = NodeReport::new("test detector", "detector name", "123", properties);
        let html_report = HtmlNodeReport::from_node_report(&report, 0);
        assert_eq!(html_report.node_name, "detector name");
        assert_eq!(html_report.node_type, "test detector");
        assert_eq!(html_report.uuid, "123");
        assert!(!html_report.show_item);
        assert!(html_report.notes.is_empty());
        let html_props = html_report.props;

        assert_eq!(html_props[0].name, "test1");
        assert_eq!(html_props[0].description, "desc1");
        assert_eq!(html_props[0].prop_value, "1.000000");
        Ok(())
    }

    #[test]
    fn test_proptype_to_html() -> ReporterResult<()> {
        assert_eq!(
            proptype_to_html(&Proptype::String("Test".into()), "id", "property_name", 0)?,
            "Test".to_string()
        );
        assert_eq!(
            proptype_to_html(&Proptype::I32(-14), "id", "property_name", 0)?,
            "-14".to_string()
        );
        assert_eq!(
            proptype_to_html(&Proptype::F64(-3.1415926537), "id", "property_name", 0)?,
            "-3.141593".to_string()
        );
        assert_eq!(
            proptype_to_html(&Proptype::Bool(true), "id", "property_name", 0)?,
            "true".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::SpectrometerType(SpectrometerType::HR2000),
                "id",
                "property_name",
                0
            )?,
            "Ocean Optics HR2000".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::from(Material::default()),
                "id",
                "property_name",
                0
            )?,
            "Default Glass (v0)".to_string()
        );
        let material_id = Uuid::nil();
        assert_eq!(
            proptype_to_html(
                &Proptype::Material(AssetRef::<Material>::Id(material_id)),
                "id",
                "property_name",
                0
            )?,
            format!("MaterialRef({material_id})")
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::WaveFrontData(WaveFrontMap::default()),
                "id",
                "property_name",
                0
            )?,
            "<img src=\"data_0/id_property_name.png\" class=\"img-fluid\" style=\"max-height: 500pt;\" alt=\"measurement data\"/>".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::Fluence(J_per_m2!(1.234567)),
                "id",
                "property_name",
                0
            )?,
            " 123.457 μJ/cm²".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::WfLambda(0.123456, nanometer!(1054.0)),
                "id",
                "property_name",
                0
            )?,
            " 123.456 mλ, (λ =    1.054 μm)".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::Length(meter!(0.12345678)),
                "id",
                "property_name",
                0
            )?,
            " 123.457 mm".to_string()
        );
        assert_eq!(
            proptype_to_html(
                &Proptype::Energy(joule!(0.12345678)),
                "id",
                "property_name",
                0
            )?,
            " 123.457 mJ".to_string()
        );
        Ok(())
    }
}
