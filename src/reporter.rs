use chrono::{DateTime, Local};
use genpdf::{self, elements, style, Alignment, Scale};
use serde_derive::Serialize;

use crate::properties::Properties;
#[derive(Serialize)]
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    node_reports: Vec<NodeReport>,
}
impl AnalysisReport {
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            node_reports: Default::default(),
        }
    }
    pub fn add_detector(&mut self, report: NodeReport) {
        self.node_reports.push(report);
    }
}
#[derive(Serialize)]
pub struct NodeReport {
    detector_type: String,
    name: String,
    properties: Properties,
}
impl NodeReport {
    pub fn new(detector_type: String, name: String, properties: Properties) -> Self {
        Self {
            detector_type,
            name,
            properties,
        }
    }
}

pub trait PdfReportable {
    fn pdf_report(&self) -> genpdf::elements::LinearLayout;
}

pub struct ReportGenerator {
    report: AnalysisReport,
}

impl ReportGenerator {
    pub fn new(report: AnalysisReport) -> Self {
        Self { report }
    }

    pub fn generate_pdf(&self) {
        let font_family = genpdf::fonts::from_files("./fonts", "LiberationSans", None)
            .expect("Failed to load font family");
        // Create a document and set the default font family
        let mut doc = genpdf::Document::new(font_family);
        // Change the default settings
        doc.set_title("OPOSSUM Analysis report");
        // Customize the pages
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);
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
        doc.push(genpdf::elements::Break::new(2));
        let p = elements::Paragraph::default().styled_string("Detectors", style::Effect::Bold);
        doc.push(p);
        for detector in self.report.node_reports.iter() {
            let p = elements::Paragraph::default()
                .string(format!("{} - {}", detector.name, detector.detector_type));
            doc.push(p);
            doc.push(detector.properties.pdf_report());
        }
        // Render the document and write it to a file
        doc.render_to_file("./playground/output.pdf")
            .expect("Failed to write PDF file");
    }
}
