use opossum_core::{absorption::absorption_model::AbsorptionModel, light::Spectrum, micrometer};
use std::path::Path;
use strum::EnumIter;
use uom::si::length::nanometer;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};

/// Parameter descriptors for the spectral Lambert-Beer absorption model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum LambertBeerSpecParam {
    /// File path selector to import spectral data from CSV.
    CsvFile,
}

impl From<LambertBeerSpecParam> for InputParam {
    fn from(param: LambertBeerSpecParam) -> Self {
        match param {
            LambertBeerSpecParam::CsvFile => {
                // File chooser filtered to CSV extensions
                Self::FilePath("Absorption Spectrum (CSV)".to_string(), "csv".to_string())
            }
        }
    }
}

impl IntoInputDataStrings<Spectrum> for LambertBeerSpecParam {
    fn create_value_string(&self, obj: &Spectrum) -> String {
        match self {
            Self::CsvFile => {
                let count = obj.data().len();
                if count == 0 {
                    "Empty spectrum".to_string()
                } else if count == 1 {
                    // Convert the single µm data point to nm for display
                    let single_nm = micrometer!(obj.data()[0].0).get::<nanometer>();
                    format!("1 pt ({single_nm:.1} nm)")
                } else {
                    let range = obj.range();
                    let start_nm = range.start.get::<nanometer>();
                    let end_nm = range.end.get::<nanometer>();
                    format!("{count} pts ({start_nm:.1} - {end_nm:.1} nm)")
                }
            }
        }
    }

    fn create_id_string(&self) -> String {
        match self {
            Self::CsvFile => "lb_spec_csv_".to_string(),
        }
    }
}

impl IntoInputData<String, Spectrum, AbsorptionModel> for LambertBeerSpecParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut Spectrum, String) {
        move |obj: &mut Spectrum, file_path: String| {
            let path = Path::new(&file_path);

            match Spectrum::from_csv(path) {
                Ok(loaded_spectrum) => {
                    *obj = loaded_spectrum;
                }
                Err(err) => {
                    let msg = format!("Failed to parse spectrum CSV '{file_path}': {err}");
                    log::error!("{msg}");
                    OPOSSUM_UI_LOGS.write().add_log(&msg);
                }
            }
        }
    }
}
