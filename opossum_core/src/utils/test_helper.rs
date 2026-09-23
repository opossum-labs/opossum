//! various helper functions used to simplify unit tests.
//!
//! **Note**: This module is only compiled and used during testing. Hence, there might be no
//! further documentation show up.

#[cfg(test)]
pub mod test_helper {
    use crate::{
        degree,
        error::{OpmResult, OpossumError},
        geometry::geo_surface::{GeoSurface, GeoSurfaceRef},
        millimeter,
        properties::Proptype,
        reporting::analysis_report::AnalysisReport,
        utils::geom_transformation::Isometry,
    };
    use nalgebra::Point2;
    use std::sync::{Arc, Mutex};
    use uom::si::f64::Length;

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

    /// Place a surface the way a node does, at a distance along that node's axis.
    ///
    /// Going through `set_isometry` rather than the constructor matters: it is what puts a curved
    /// surface's *vertex* on the given frame instead of its center of curvature.
    ///
    /// # Arguments
    ///
    /// * `surface` - the surface to place
    /// * `frame` - the frame of the node holding it
    /// * `z` - how far along that node's axis the surface sits
    ///
    /// # Returns
    ///
    /// The placed surface, ready to be handed to a body.
    ///
    /// # Errors
    ///
    /// This function returns an error if the offset along the axis is not a valid placement.
    pub fn placed<S: GeoSurface + 'static>(
        mut surface: S,
        frame: &Isometry,
        z: Length,
    ) -> OpmResult<GeoSurfaceRef> {
        surface.set_isometry(frame.append(&Isometry::new_along_z(z)?));
        Ok(GeoSurfaceRef(Arc::new(Mutex::new(surface))))
    }

    /// The corners of an L, the smallest outline that is not convex.
    ///
    /// Counter-clockwise, and with one corner the outline turns *into* — which is what makes it
    /// worth testing against anything that walks or fills an outline.
    #[must_use]
    pub fn l_shape_corners() -> Vec<Point2<Length>> {
        vec![
            millimeter!(0.0, 0.0),
            millimeter!(20.0, 0.0),
            millimeter!(20.0, 5.0),
            millimeter!(5.0, 5.0),
            millimeter!(5.0, 20.0),
            millimeter!(0.0, 20.0),
        ]
    }
}
