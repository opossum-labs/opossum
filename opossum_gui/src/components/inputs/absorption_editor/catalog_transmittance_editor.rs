use opossum_core::{
    absorption::{
        absorption_catalog_transmittance::AbsCatTrans, absorption_model::AbsorptionModel,
    },
    light::Spectrum,
    micrometer,
};
use std::path::Path;
use strum::EnumIter;
use uom::si::{
    f64::Length,
    length::{meter, nanometer},
};

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::inputs::{InputParam, IntoInputData, IntoInputDataStrings},
};

/// Parameter descriptors for the glass catalog internal transmittance model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum CatalogTransmittanceParam {
    /// Reference sample thickness (e.g., 10 mm or 25 mm).
    ReferenceThickness,
    /// File path selector to import tabulated internal transmittance data.
    CsvFile,
}

impl From<CatalogTransmittanceParam> for InputParam {
    fn from(param: CatalogTransmittanceParam) -> Self {
        match param {
            CatalogTransmittanceParam::ReferenceThickness => {
                Self::SIUnit("Reference thickness".to_string(), "m".to_string())
            }
            CatalogTransmittanceParam::CsvFile => {
                Self::FilePath("Catalog Transmittance (CSV)".to_string(), "csv".to_string())
            }
        }
    }
}

impl IntoInputDataStrings<AbsCatTrans> for CatalogTransmittanceParam {
    fn create_value_string(&self, obj: &AbsCatTrans) -> String {
        match self {
            Self::ReferenceThickness => {
                // Returns the value in meters as a string for NodeConfigUnitInput
                obj.reference_thickness().get::<meter>().to_string()
            }
            Self::CsvFile => {
                let count = obj.spectrum().data().len();
                if count == 0 {
                    "Empty spectrum".to_string()
                } else if count == 1 {
                    let single_nm = micrometer!(obj.spectrum().data()[0].0).get::<nanometer>();
                    format!("1 pt ({single_nm:.1} nm)")
                } else {
                    let range = obj.spectrum().range();
                    let start_nm = range.start.get::<nanometer>();
                    let end_nm = range.end.get::<nanometer>();
                    format!("{count} pts ({start_nm:.1} - {end_nm:.1} nm)")
                }
            }
        }
    }

    fn create_id_string(&self) -> String {
        match self {
            Self::ReferenceThickness => "cat_trans_thickness_".to_string(),
            Self::CsvFile => "cat_trans_csv_".to_string(),
        }
    }
}

impl IntoInputData<String, AbsCatTrans, AbsorptionModel> for CatalogTransmittanceParam {
    fn setter_from_obj(&self) -> impl FnMut(&mut AbsCatTrans, String) {
        let this = *self;
        move |obj: &mut AbsCatTrans, val: String| match this {
            Self::ReferenceThickness => {
                if let Ok(thickness_val) = val.parse::<f64>()
                    && thickness_val > 0.0
                    && thickness_val.is_finite()
                {
                    let length = Length::new::<meter>(thickness_val);
                    let _ = obj.set_reference_thickness(length);
                }
            }
            Self::CsvFile => {
                let path = Path::new(&val);
                match Spectrum::from_csv(path) {
                    Ok(loaded_spectrum) => {
                        obj.set_spectrum(loaded_spectrum);
                    }
                    Err(err) => {
                        let msg = format!("Failed to parse catalog CSV '{val}': {err}");
                        log::error!("{msg}");
                        OPOSSUM_UI_LOGS.write().add_log(&msg);
                    }
                }
            }
        }
    }
}
