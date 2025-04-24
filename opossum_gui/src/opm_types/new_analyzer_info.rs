use super::analyzer_type::AnalyzerType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct NewAnalyzerInfo {
    analyzer_type: AnalyzerType,
    gui_position: (i32, i32, i32),
}
impl NewAnalyzerInfo {
    #[must_use]
    pub const fn new(analyzer_type: AnalyzerType, gui_position: (i32, i32, i32)) -> Self {
        Self {
            analyzer_type,
            gui_position,
        }
    }
}
