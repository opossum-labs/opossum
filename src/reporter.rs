use chrono::{DateTime, Local};
use genpdf::{self, fonts::Builtin};
pub struct AnalysisReport {
    opossum_version: String,
    analysis_timestamp: DateTime<Local>,
}

impl AnalysisReport {
    pub fn new(opossum_version: String, analysis_timestamp: DateTime<Local>) -> Self {
        Self {
            opossum_version,
            analysis_timestamp,
        }
    }
}

pub struct ReportGenerator {
    report: AnalysisReport,
}

impl ReportGenerator {
    pub fn new(report: AnalysisReport) -> Self {
        Self { report }
    }

    pub fn generate_pdf() {
        let font_family = genpdf::fonts::from_files("./fonts", "LiberationSans", None)
            .expect("Failed to load font family");
        // Create a document and set the default font family
        let mut doc = genpdf::Document::new(font_family);
        // Change the default settings
        doc.set_title("Demo document");
        // Customize the pages
        let mut decorator = genpdf::SimplePageDecorator::new();
        decorator.set_margins(10);
        doc.set_page_decorator(decorator);
        // Add one or more elements
        doc.push(genpdf::elements::Paragraph::new("This is a demo document."));
        // Render the document and write it to a file
        doc.render_to_file("./playground/output.pdf")
            .expect("Failed to write PDF file");
    }
}
