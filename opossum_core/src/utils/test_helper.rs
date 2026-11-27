//! various helper functions used to simplify unit tests.
//!
//! **Note**: This module is only compiled and used during testing. Hence, there might be no
//! further documentation show up.

#[cfg(test)]
pub mod helper {
    use crate::{
        analyzers::RayTraceConfig,
        degree,
        distributions::position::Hexapolar,
        error::{OpmResult, OpossumError},
        joule,
        light::Rays,
        millimeter, nanometer,
        nodes::{NodeGroup, fluence_detector::fluence_data::FluenceData},
        properties::Proptype,
        reporting::analysis_report::AnalysisReport,
        utils::geom_transformation::Isometry,
    };
    use nalgebra::DVector;
    use uom::si::{f64::Length, length::millimeter, radiant_exposure::joule_per_square_centimeter};

    /// Places `scenery` along its optical axis as a document does before it analyzes it, with the
    /// setup aligned in vacuum (see [`NodeGroup::reposition`]): an analysis that traces rays needs
    /// its nodes placed.
    pub fn reposition_in_vacuum(scenery: &mut NodeGroup) -> OpmResult<()> {
        scenery.reposition(&RayTraceConfig::default().for_positioning())
    }

    /// Read the energy an [`EnergyMeter`](crate::nodes::EnergyMeter) recorded from an analysis
    /// report.
    ///
    /// The reading is fetched from the report rather than from the node because that is the only
    /// place it is exposed - which makes this the way any test asks "how much energy came out at
    /// the end", whatever kind of analysis produced the report.
    ///
    /// # Arguments
    ///
    /// * `report` - the report of a finished analysis.
    ///
    /// # Returns
    ///
    /// The recorded energy in joule.
    ///
    /// # Errors
    ///
    /// Returns an error if the report contains no energy reading at all.
    pub fn metered_energy(report: &AnalysisReport) -> OpmResult<f64> {
        report
            .node_reports()
            .iter()
            .find_map(|node_report| match node_report.properties().get("Energy") {
                Ok(Proptype::Energy(energy)) => Some(energy.value),
                _ => None,
            })
            .ok_or_else(|| OpossumError::Other("no energy reading in the report".into()))
    }

    pub fn check_logs(level: log::Level, expected_warnings: Vec<&str>) {
        testing_logger::validate(|captured_logs| {
            let captured_logs: Vec<_> = captured_logs.iter().filter(|l| l.level == level).collect();
            assert_eq!(
                captured_logs.len(),
                expected_warnings.len(),
                "expected # of warnings do not match: {} != {}. Got warnings: {:?}",
                captured_logs.len(),
                expected_warnings.len(),
                captured_logs
                    .iter()
                    .map(|l| l.body.as_str())
                    .collect::<Vec<_>>()
            );
            for log in captured_logs.iter().zip(expected_warnings.clone()) {
                assert_eq!(log.0.body, log.1);
            }
        });
    }

    /// Assert that a warning containing `part` was logged since `testing_logger::setup()`.
    pub fn assert_warned(part: &str) {
        testing_logger::validate(|logs| {
            assert!(
                logs.iter()
                    .any(|log| log.level == log::Level::Warn && log.body.contains(part)),
                "no warning containing '{part}' in {:?}",
                logs.iter().map(|log| log.body.as_str()).collect::<Vec<_>>()
            );
        });
    }

    /// A frame well away from the origin and turned about all three axes.
    ///
    /// Geometry expressed in a component's own frame must not depend on where that component sits,
    /// so anything making that claim is asked here for somewhere awkward to sit.
    ///
    /// # Returns
    ///
    /// A placement that is neither at the origin nor axis-aligned.
    ///
    /// # Errors
    ///
    /// This function returns an error if the hard-coded placement is rejected, which cannot happen.
    pub fn somewhere_else() -> OpmResult<Isometry> {
        Isometry::new(millimeter!(12.0, -4.0, 80.0), degree!(3.0, -7.0, 20.0))
    }
    /// A collimated bundle of 1 J at 1000 nm along z, of `rings` hexapolar rings 5 mm in radius.
    ///
    /// # Errors
    ///
    /// This function returns an error if the bundle cannot be built, which cannot happen.
    pub fn hexapolar_bundle(rings: u8) -> OpmResult<Rays> {
        Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), rings)?,
        )
    }
    /// The fluence, in J/cm², of a flat-top beam of 1 J in `rings` hexapolar rings of the given
    /// radius in mm: each ring stands for an annulus one ring spacing wide, so the outermost one
    /// reaches half a spacing beyond the radius.
    #[must_use]
    pub fn flat_top_fluence(radius: f64, rings: u8) -> f64 {
        let reach = radius * (1.0 + 0.5 / f64::from(rings)) / 10.0;
        1.0 / (std::f64::consts::PI * reach * reach)
    }
    /// The point of the given map nearest to `(x, y)`, in mm: where it lies, in mm, and its
    /// value, in J/cm².
    ///
    /// # Errors
    ///
    /// This function returns an error if the axes of the map cannot be built.
    pub fn point_near(map: &FluenceData, x: f64, y: f64) -> OpmResult<(f64, f64, f64)> {
        let (xs, ys, values) = map.get_fluence_distribution();
        let nearest = |axis: &DVector<Length>, at: f64| {
            axis.iter()
                .map(|c| c.get::<millimeter>())
                .enumerate()
                .min_by(|(_, a), (_, b)| (*a - at).abs().total_cmp(&(*b - at).abs()))
                .ok_or_else(|| OpossumError::Other("the map is empty".into()))
        };
        let ((column, x), (row, y)) = (nearest(&xs, x)?, nearest(&ys, y)?);
        let value = values
            .get((row, column))
            .map(|fluence| fluence.get::<joule_per_square_centimeter>())
            .ok_or_else(|| OpossumError::Other("the map is empty".into()))?;
        Ok((x, y, value))
    }
    /// The value of the given map at the point nearest to `(x, y)`, in mm, in J/cm².
    ///
    /// # Errors
    ///
    /// This function returns an error if the axes of the map cannot be built.
    pub fn fluence_near(map: &FluenceData, x: f64, y: f64) -> OpmResult<f64> {
        Ok(point_near(map, x, y)?.2)
    }
}
