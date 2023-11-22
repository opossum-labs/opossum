#![warn(missing_docs)]
//! Module for generating analysis reports in PDF format.

use chrono::{DateTime, Local};
use genpdf::{self, elements, style, Alignment, Scale};
use serde_derive::Serialize;
use std::path::Path;

use crate::{error::OpmResult, properties::Properties, OpticScenery};
#[derive(Serialize, Debug)]
/// Structure for storing data being integrated in an analysis report.
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    scenery: Option<OpticScenery>,
    node_reports: Vec<NodeReport>,
}
impl AnalysisReport {
    /// Creates a new [`AnalysisReport`].
    #[must_use]
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            scenery: None,
            node_reports: Vec::default(),
        }
    }
    /// Add an [`OpticScenery`] to this [`AnalysisReport`].
    ///
    /// This function is called internally [`OpticScenery`] for adding itself to the report.
    pub fn add_scenery(&mut self, scenery: &OpticScenery) {
        self.scenery = Some(scenery.clone());
    }
    /// Add an (detector) [`NodeReport`] to this [`AnalysisReport`].
    ///
    /// After analysis of an [`OpticScenery`], ech node can generate a [`NodeReport`] using the `Optical::report` trait function.
    /// While assembling a report this function adds the node data to it. This is mostly interesting for detector nodes which deliver
    /// their particular analysis result.
    pub fn add_detector(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }
}
#[derive(Serialize, Debug)]
/// Structure for storing (detector-)node specific data to be integrated in the [`AnalysisReport`].
pub struct NodeReport {
    detector_type: String,
    name: String,
    properties: Properties,
}
impl NodeReport {
    /// Creates a new [`NodeReport`].
    #[must_use]
    pub fn new(detector_type: &str, name: &str, properties: Properties) -> Self {
        Self {
            detector_type: detector_type.to_owned(),
            name: name.to_owned(),
            properties,
        }
    }
}

/// Trait for providing information to be integrated in an PDF analysis report.
pub trait PdfReportable {
    /// Return a `genpdf`-based PDF component to be integrated in an analysis report.
    ///
    /// # Errors
    ///
    /// This function will return an error if the underlying particular implementation produces an error.
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout>;
}

/// PDF analysis report generator
///
/// This report generator delivers a PDF file containing the analysis report based on the provided [`AnalysisReport`].
pub struct ReportGenerator {
    report: AnalysisReport,
}

impl ReportGenerator {
    /// Creates a new [`ReportGenerator`].
    #[must_use]
    pub const fn new(report: AnalysisReport) -> Self {
        Self { report }
    }
    fn add_report_title(&self, doc: &mut genpdf::Document) {
        let image = elements::Image::from_path("logo/Logo_square.png")
            .expect("Failed to load image")
            .with_scale(Scale::new(0.2, 0.2))
            .with_alignment(Alignment::Center);
        doc.push(image);
        let p = elements::Paragraph::default()
            .styled_string("Analysis Report", style::Effect::Bold)
            .aligned(genpdf::Alignment::Center);
        // Add one or more elements
        doc.push(p);
        let mut table = elements::TableLayout::new(vec![1, 1]);
        let mut table_row = table.row();
        table_row.push_element(
            elements::Paragraph::default()
                .styled_string(
                    format!("OPOSSUM v{}", self.report.opossum_version),
                    style::Style::new().with_font_size(8),
                )
                .aligned(Alignment::Left),
        );
        table_row.push_element(
            elements::Paragraph::default()
                .styled_string(
                    format!(
                        "Date: {}",
                        self.report.analysis_timestamp.format("%Y-%m-%d %H:%M:%S")
                    ),
                    style::Style::new().with_font_size(8),
                )
                .aligned(Alignment::Right),
        );
        table_row.push().unwrap();
        doc.push(table);
    }
    fn add_scenery_report(&self, doc: &mut genpdf::Document) -> OpmResult<()> {
        if let Some(scenery) = &self.report.scenery {
            doc.push(genpdf::elements::Break::new(2));
            let p = elements::Paragraph::default().styled_string(
                format!("Scenery: {}", scenery.description()),
                style::Effect::Bold,
            );
            doc.push(p);
            doc.push(scenery.pdf_report()?);
        }
        Ok(())
    }
    fn add_node_reports(&self, doc: &mut genpdf::Document) -> OpmResult<()> {
        if !self.report.node_reports.is_empty() {
            doc.push(genpdf::elements::Break::new(2));
            let p = elements::Paragraph::default().styled_string("Detectors", style::Effect::Bold);
            doc.push(p);
            for detector in &self.report.node_reports {
                doc.push(genpdf::elements::Break::new(1));
                let p = elements::Paragraph::default()
                    .string(format!("{} - {}", detector.name, detector.detector_type));
                doc.push(p);
                doc.push(detector.properties.pdf_report()?);
            }
        }
        Ok(())
    }
    /// Generate a OPOSSUM analysis report as PDF.
    ///
    /// # Errors
    ///
    /// This function will return an error if the document generation fails because e.g.
    ///   - fonts are not found
    ///   - the file could not be generated on disk (disk full, not writable, etc...)
    ///   - individual erros while generating sub components of the report
    pub fn generate_pdf(&self, path: &Path) -> OpmResult<()> {
        let font_family = genpdf::fonts::from_files("./opossum/fonts", "LiberationSans", None)
            .map_err(|e| format!("failed to load font family: {e}"))?;
        // Create a document and set the default font family
        let mut doc = genpdf::Document::new(font_family);
        // Change the default settings
        doc.set_title("OPOSSUM Analysis report");
        // Customize the pages
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);

        self.add_report_title(&mut doc);
        self.add_scenery_report(&mut doc)?;
        doc.push(genpdf::elements::PageBreak::new());
        self.add_node_reports(&mut doc)?;
        doc.render_to_file(path)
            .map_err(|e| format!("failed to write file: {e}"))?;
        Ok(())
    }
}
