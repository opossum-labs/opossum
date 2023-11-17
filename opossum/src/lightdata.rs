//! Data structures containing the light information flowing between [`Opticals`](crate::optical::Optical).
use plotters::coord::Shift;
use plotters::prelude::{DrawingArea, DrawingBackend};
use serde_derive::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::Path;
use uom::fmt::DisplayStyle::Abbreviation;
use uom::si::{energy::joule, f64::Energy};

use crate::error::OpmResult;
use crate::plottable::Plottable;
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
    /// This function will return an error if `to_svg_plot` fails for the case that the plot area cannot be filled with a background colour.
    pub fn export(&self, f_path: &Path) -> OpmResult<()> {
        match self {
            Self::Energy(d) => {
                d.to_svg_plot(f_path)?;
            }
            _ => println!("no export function defined for this type of LightData"),
        }
        Ok(())
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
pub struct DataEnergy {
    pub spectrum: Spectrum,
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
