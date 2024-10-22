#![warn(missing_docs)]
//! Data structures containing the light information flowing between [`OpticNode`s](crate::optic_node::OpticNode).
use crate::{error::OpmResult, joule, nodes::FilterType, rays::Rays, spectrum::Spectrum};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::si::{energy::joule, f64::Energy};

/// Data structure defining the light properties. The actuals data type used depends on the
/// [`AnalyzerType`](crate::analyzers::AnalyzerType). For example, an energy analysis ([`LightData::Energy`]) only
/// contains a [`Spectrum`] information, while a geometric analysis ([`LightData::Geometric`]) constains a set of optical
/// ray data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightData {
    /// data type used for energy analysis.
    Energy(DataEnergy),
    /// data type used for geometric optics analysis (ray tracing)
    Geometric(Rays),
    /// data type used for ghost focus analysis (back- and forth ray-tracing)
    GhostFocus(Vec<Rays>),
    /// placeholder value for future Fourier optics analysis, nothing implementd yet.
    Fourier,
}

impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy(e) => {
                let ef = Energy::format_args(joule, Abbreviation);
                write!(f, "Energy: {}", ef.with(joule!(e.spectrum.total_energy())))
            }
            _ => write!(f, "No display defined for this type of LightData"),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Data structure for storing [`LightData::Energy`] data.
///
/// It currently only contains a [`Spectrum`].
pub struct DataEnergy {
    /// The spectrum for energy analysis.
    pub spectrum: Spectrum,
}
impl DataEnergy {
    /// Filter this [`DataEnergy`] by a given `filter_type`.
    ///
    /// Modify the overall energy of the underlying spectrum depneding on the concrete filter type. For a [`FilterType::Constant`] simple scale all
    /// spectrum values by the given factor. For [`FilterType::Spectrum`] multiply both spectra.
    /// # Errors
    ///
    /// This function will return an error if [`FilterType::Constant`] is used and the transmission value is outside the interval `[0.0;1.0]`.
    pub fn filter(&mut self, filter_type: &FilterType) -> OpmResult<()> {
        match filter_type {
            FilterType::Constant(t) => self.spectrum.scale_vertical(t)?,
            FilterType::Spectrum(s) => {
                self.spectrum.filter(s);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use crate::{properties::Proptype, spectrum_helper::create_visible_spec, utils::EnumProxy};

    use super::*;
    use assert_matches::assert_matches;
    #[test]
    fn display_unknown() {
        assert_eq!(
            format!("{}", LightData::Fourier),
            "No display defined for this type of LightData"
        );
    }
    #[test]
    fn display_energy() {
        let ld = LightData::Energy(DataEnergy {
            spectrum: create_visible_spec(),
        });
        assert_eq!(format!("{ld}"), "Energy: 0 J");
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", LightData::Fourier), "Fourier");
    }
    // #[test]
    // fn export_wrong() {
    //     assert!(LightData::Fourier.export(Path::new("")).is_err());
    // }
    #[test]
    fn from() {
        let ld = Proptype::from(EnumProxy::<Option<LightData>> {
            value: Some(LightData::Fourier),
        });
        assert_matches!(ld, Proptype::LightData(_));
    }
    // #[test]
    // fn data_energy_pdf_report() {
    //     assert!(DataEnergy {
    //         spectrum: create_visible_spec()
    //     }
    //     .pdf_report()
    //     .is_ok());
    // }
}
