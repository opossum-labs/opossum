//! Data structure for holding a fluence map.
use std::ops::Range;

use super::Fluence;
use crate::{
    error::OpmResult,
    joule,
    plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType, Plottable},
    properties::Proptype,
    utils::griddata::linspace,
    J_per_cm2,
};
use nalgebra::{DMatrix, DVector};
use plotters::style::RGBAColor;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Energy, Length},
    length::{meter, millimeter},
    radiant_exposure::joule_per_square_centimeter,
};

impl From<FluenceData> for Proptype {
    fn from(value: FluenceData) -> Self {
        Self::FluenceData(value)
    }
}
/// Struct to hold the fluence information of a beam
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FluenceData {
    /// peak fluence of the beam
    peak: Fluence,
    /// average fluence of the beam
    average: Fluence,
    /// 2d fluence distribution of the beam.
    interp_distribution: DMatrix<Fluence>,
    /// x coordinates of the fluence distribution
    x_range: Range<Length>,
    /// y coordinates of the fluence distribution
    y_range: Range<Length>,
}
impl FluenceData {
    /// Constructs a new [`FluenceData`] struct
    #[must_use]
    pub fn new(
        average: Fluence,
        interp_distribution: DMatrix<Fluence>,
        x_range: Range<Length>,
        y_range: Range<Length>,
    ) -> Self {
        let peak_fluence =
            interp_distribution
                .iter()
                .fold(J_per_cm2!(f64::NEG_INFINITY), |arg0, v| {
                    if v.is_finite() {
                        Fluence::max(arg0, *v)
                    } else {
                        arg0
                    }
                });
        Self {
            peak: peak_fluence,
            average,
            interp_distribution,
            x_range,
            y_range,
        }
    }
    /// Returns the fluence distribution and the corresponding x and y axes in a tuple (x, y, distribution)
    ///
    /// # Panics
    ///
    /// This function panics if the linear axis vector (linspace) could not be generated.
    #[must_use]
    pub fn get_fluence_distribution(&self) -> (DVector<Length>, DVector<Length>, DMatrix<Fluence>) {
        (
            linspace(
                self.x_range.start.value,
                self.x_range.end.value,
                self.interp_distribution.ncols(),
            )
            .unwrap()
            .map(Length::new::<meter>),
            linspace(
                self.y_range.start.value,
                self.y_range.end.value,
                self.interp_distribution.nrows(),
            )
            .unwrap()
            .map(Length::new::<meter>),
            self.interp_distribution.clone(),
        )
    }
    /// Returns length of the x data points (columns)
    #[must_use]
    pub fn len_x(&self) -> usize {
        self.interp_distribution.ncols()
    }
    /// Returns length of the y data points (rows)
    #[must_use]
    pub fn len_y(&self) -> usize {
        self.interp_distribution.nrows()
    }
    /// Returns the shape of the interpolation distribution in pixels
    #[must_use]
    pub fn shape(&self) -> (usize, usize) {
        self.interp_distribution.shape()
    }
    /// Returns the peak fluence of this [`FluenceData`].
    #[must_use]
    pub fn peak(&self) -> Fluence {
        self.peak
    }
    /// Returns the average fluence of this [`FluenceData`].
    #[must_use]
    pub fn average(&self) -> Fluence {
        self.average
    }
    /// Returns the total energy of this [`FluenceData`].
    #[must_use]
    pub fn total_energy(&self) -> Energy {
        #[allow(clippy::cast_precision_loss)]
        let dx = (self.x_range.end - self.x_range.start) / (self.len_x() as f64);
        #[allow(clippy::cast_precision_loss)]
        let dy = (self.y_range.end - self.y_range.start) / (self.len_y() as f64);
        let area = dx * dy;
        let mut energy = joule!(0.0);
        for fluence in &self.interp_distribution {
            if !fluence.is_nan() {
                energy += area * (*fluence);
            }
        }
        energy
    }
}

impl Plottable for FluenceData {
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()> {
        plt_params
            .set(&PlotArgs::XLabel("distance in mm".into()))?
            .set(&PlotArgs::YLabel("distance in mm".into()))?
            .set(&PlotArgs::CBarLabel("fluence in J/cm²".into()))?
            .set(&PlotArgs::PlotSize((800, 800)))?
            .set(&PlotArgs::ExpandBounds(false))?;

        Ok(())
    }

    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType {
        PlotType::ColorMesh(plt_params.clone())
    }

    fn get_plot_series(
        &self,
        plt_type: &mut PlotType,
        _legend: bool,
    ) -> OpmResult<Option<Vec<PlotSeries>>> {
        let (nrows, ncols) = self.interp_distribution.shape();

        match plt_type {
            PlotType::ColorMesh(_) => {
                let plt_data = PlotData::ColorMesh {
                    x_dat_n: linspace(
                        self.x_range.start.get::<millimeter>(),
                        self.x_range.end.get::<millimeter>(),
                        self.interp_distribution.ncols(),
                    )
                    .unwrap(),
                    y_dat_m: linspace(
                        self.y_range.start.get::<millimeter>(),
                        self.y_range.end.get::<millimeter>(),
                        self.interp_distribution.nrows(),
                    )
                    .unwrap(),
                    z_dat_nxm: DMatrix::from_iterator(
                        nrows,
                        ncols,
                        self.interp_distribution
                            .iter()
                            .map(uom::si::f64::RadiantExposure::get::<joule_per_square_centimeter>),
                    ),
                };
                let plt_series = PlotSeries::new(&plt_data, RGBAColor(255, 0, 0, 1.), None);
                Ok(Some(vec![plt_series]))
            }
            _ => Ok(None),
        }
    }
}
