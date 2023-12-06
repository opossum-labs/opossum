#![warn(missing_docs)]
//! Data structures containing the light information flowing between [`Opticals`](crate::optical::Optical).
use plotters::coord::Shift;
use plotters::prelude::{DrawingArea, DrawingBackend};
use serde_derive::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::Path;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::si::{energy::joule, f64::Energy};

use crate::error::{OpmResult, OpossumError};
use crate::nodes::FilterType;
use crate::plottable::{Plottable, PlotType};
use crate::properties::Proptype;
use crate::rays::Rays;
use crate::reporter::PdfReportable;
use crate::spectrum::Spectrum;

/// Data structure defining the light properties. The actuals data type used depends on the
/// [`AnalyzerType`](crate::analyzer::AnalyzerType). For example, an energy analysis ([`LightData::Energy`]) only
/// contains a [`Spectrum`] information, while a geometric analysis ([`LightData::Geometric`]) constains a set of optical
/// ray data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightData {
    /// data type used for energy analysis.
    Energy(DataEnergy),
    /// data type used for geometric optics analysis (ray tracing)
    Geometric(Rays),
    /// placeholder value for future Fourier optics analysis, nothing implementd yet.
    Fourier,
}
impl LightData {
    /// Export [`LightData`] to a specific file format
    /// # Attributes
    /// * `f_path`: path to the file destination
    ///
    /// # Errors
    /// This function will return an error if
    ///  - `to_svg_plot` fails for [`LightData::Energy`] the case that the plot area cannot be filled with a background colour.
    ///  - no export function ist defined for the conrecte type of [`LightData`]
    pub fn export(&self, f_path: &Path, plot_type: PlotType) -> OpmResult<()> {
        match self {
            Self::Energy(d) => {
                d.to_svg_plot(f_path, plot_type)?;
                Ok(())
            }
            Self::Geometric(d) => {
                d.to_svg_plot(f_path, plot_type)?;
                Ok(())
            }
            Self::Fourier => Err(OpossumError::Other(
                "export: no export function defined for this type of LightData".into(),
            )),
        }
    }
}
impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy(e) => {
                let ef = Energy::format_args(joule, Abbreviation);
                write!(
                    f,
                    "Energy: {}",
                    ef.with(Energy::new::<joule>(e.spectrum.total_energy()))
                )
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
impl PdfReportable for DataEnergy {
    fn pdf_report(&self) -> OpmResult<genpdf::elements::LinearLayout> {
        self.spectrum.pdf_report()
    }
}
impl Plottable for DataEnergy {
    fn chart<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()> {
        self.spectrum.chart(root)
    }
}
impl From<Option<LightData>> for Proptype {
    fn from(value: Option<LightData>) -> Self {
        Self::LightData(value)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{spectrum::create_visible_spec, plottable::PlotType};
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
    #[test]
    fn export_wrong() {
        assert!(LightData::Fourier.export(Path::new(""), PlotType::ColorMesh).is_err());
    }
    #[test]
    fn from() {
        let ld = Proptype::from(Some(LightData::Fourier));
        assert_matches!(ld, Proptype::LightData(_));
    }
}
