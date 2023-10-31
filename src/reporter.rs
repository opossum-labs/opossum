use chrono::{DateTime, Local};
use genpdf::{self, elements, style, Alignment, Scale};

use crate::properties::Properties;
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
    detector_reports: Vec<DetectorReport>,
}
impl AnalysisReport {
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
            detector_reports: Default::default(),
        }
    }
    pub fn add_detector(&mut self, report: DetectorReport) {
        self.detector_reports.push(report);
    }
}
pub struct DetectorReport {
    detector_type: String,
    name: String,
    properties: Properties,
}
impl DetectorReport {
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

        let p = elements::Paragraph::default().styled_string(
            format!(
                "OPOSSUM version: {}, Date: {}",
                self.report.opossum_version, self.report.analysis_timestamp
            ),
            style::Style::new().with_font_size(8),
        );
        doc.push(p);
        doc.push(genpdf::elements::Break::new(2));
        let p = elements::Paragraph::default().styled_string("Detectors", style::Effect::Bold);
        doc.push(p);
        for detector in self.report.detector_reports.iter() {
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
