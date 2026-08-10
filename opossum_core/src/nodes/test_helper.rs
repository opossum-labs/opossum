#[cfg(test)]
pub mod test_helper {
    use crate::{
        analyzers::{
            RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        apertures::{ApertureShape, ApertureType, CircleShape},
        core_optics::{NodeAttrExt, OpticNode, OpticNodeExt, PortType},
        distributions::position::Hexapolar,
        error::OpmResult,
        joule,
        light::{LightData, LightResult, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        prelude::Aperture,
        utils::{geom_transformation::Isometry, test_helper::test_helper::check_logs},
    };
    use nalgebra::Vector3;
    use uom::si::{energy::joule, length::millimeter};
    pub fn test_inverted<T: Default + OpticNode>() -> OpmResult<()> {
        let mut node = T::default();
        node.set_inverted(true)?;
        assert_eq!(node.inverted(), true);
        Ok(())
    }
    pub fn test_set_aperture<T: Default + OpticNode>(
        input_port_name: &str,
        output_port_name: &str,
    ) {
        let mut node = T::default();
        let aperture = Aperture::default();
        assert!(
            node.set_aperture(&PortType::Input, input_port_name, &aperture)
                .is_ok()
        );
        assert!(
            node.set_aperture(&PortType::Input, output_port_name, &aperture)
                .is_err()
        );
        assert!(
            node.set_aperture(&PortType::Input, "no port", &aperture)
                .is_err()
        );
        assert!(
            node.set_aperture(&PortType::Output, input_port_name, &aperture)
                .is_err()
        );
        assert!(
            node.set_aperture(&PortType::Output, output_port_name, &aperture)
                .is_ok()
        );
        assert!(
            node.set_aperture(&PortType::Output, "no port", &aperture)
                .is_err()
        );
    }
    pub fn test_analyze_empty<T: Default + AnalysisEnergy>() -> OpmResult<()> {
        let mut node = T::default();
        let input = LightResult::default();
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    pub fn test_analyze_wrong_data_type<T: Default + AnalysisRayTrace>(
        input_port_name: &str,
    ) -> OpmResult<()> {
        let mut node = T::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        assert!(
            node.ports()
                .names(&PortType::Input)
                .contains(&(input_port_name.into())),
            "wrong input port name used"
        );
        input.insert(input_port_name.into(), input_light.clone());
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
        Ok(())
    }
    pub fn test_analyze_apodization_warning<T: Default + AnalysisRayTrace>() -> OpmResult<()> {
        testing_logger::setup();
        let mut node = T::default();
        node.set_isometry(Isometry::identity())?;
        let config = CircleShape::new(millimeter!(1.0))?;
        node.set_aperture(
            &PortType::Input,
            "input_1",
            &Aperture::new(
                ApertureShape::BinaryCircle(config),
                ApertureType::Hole,
                None,
                None,
            )?,
        )?;
        let mut input = LightResult::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3)?,
        )?;
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        let msg = format!(
            "Rays have been apodized at input aperture of '{}' ({}). Results might not be accurate.",
            node.node_attr().name(),
            node.node_attr().node_type()
        );
        check_logs(log::Level::Warn, vec![&msg]);
        Ok(())
    }
    /// Number of scalars captured per ray by [`ray_bundle_snapshot`].
    const SNAPSHOT_WIDTH: usize = 8;

    /// Deterministic ray bundle for the volume-propagation regression tests.
    ///
    /// The three rays are chosen so that the whole entry surface → volume → exit surface path is
    /// exercised: one on-axis ray (normal incidence), one collimated ray offset in x (oblique
    /// incidence on a curved surface), and one ray offset in y that is additionally tilted (so the
    /// refraction is not confined to a single plane).
    ///
    /// # Returns
    ///
    /// A [`Rays`] bundle of exactly three rays at 1053 nm carrying 1 J each.
    ///
    /// # Errors
    ///
    /// Returns an error if a [`Ray`] cannot be constructed from the hard-coded parameters.
    pub fn volume_regression_rays() -> OpmResult<Rays> {
        let mut rays = Rays::default();
        rays.add_ray(Ray::new_collimated(
            millimeter!(0.0, 0.0, 0.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        rays.add_ray(Ray::new_collimated(
            millimeter!(5.0, 0.0, 0.0),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        rays.add_ray(Ray::new(
            millimeter!(0.0, -4.0, 0.0),
            Vector3::new(0.05, 0.1, 1.0).normalize(),
            nanometer!(1053.0),
            joule!(1.0),
        )?);
        Ok(rays)
    }

    /// Capture the full state of every ray in a bundle as plain numbers.
    ///
    /// Each entry is `[x, y, z, dx, dy, dz, energy, path_length]` with lengths in millimeter and
    /// the energy in joule. This is deliberately exhaustive: the volume-propagation regression
    /// tests use it to pin the current behaviour down completely, so that a refactoring of the
    /// entry/exit surface sequence cannot change any ray unnoticed.
    ///
    /// # Arguments
    ///
    /// * `rays` - the ray bundle to capture.
    ///
    /// # Returns
    ///
    /// One array of [`SNAPSHOT_WIDTH`] scalars per ray, in bundle order.
    #[must_use]
    pub fn ray_bundle_snapshot(rays: &Rays) -> Vec<[f64; SNAPSHOT_WIDTH]> {
        rays.iter()
            .map(|ray| {
                let pos = ray.position();
                let dir = ray.direction();
                [
                    pos.x.get::<millimeter>(),
                    pos.y.get::<millimeter>(),
                    pos.z.get::<millimeter>(),
                    dir.x,
                    dir.y,
                    dir.z,
                    ray.energy().get::<joule>(),
                    ray.path_length().get::<millimeter>(),
                ]
            })
            .collect()
    }

    /// Compare a ray-bundle snapshot against previously recorded reference values.
    ///
    /// # Arguments
    ///
    /// * `actual` - snapshot taken from the current run, see [`ray_bundle_snapshot`].
    /// * `expected` - reference values recorded when the behaviour was last accepted.
    ///
    /// # Panics
    ///
    /// Panics if the number of rays differs or if any scalar deviates by more than 1e-9. The
    /// panic message contains the current values as a paste-ready literal, so an intentional
    /// change of behaviour can be re-baselined without running a separate dump.
    pub fn assert_ray_bundle_snapshot(
        actual: &[[f64; SNAPSHOT_WIDTH]],
        expected: &[[f64; SNAPSHOT_WIDTH]],
    ) {
        const LABELS: [&str; SNAPSHOT_WIDTH] = [
            "x",
            "y",
            "z",
            "dir x",
            "dir y",
            "dir z",
            "energy",
            "path length",
        ];
        let mismatch = actual.len() != expected.len()
            || actual
                .iter()
                .zip(expected)
                .any(|(actual_ray, expected_ray)| {
                    actual_ray
                        .iter()
                        .zip(expected_ray)
                        .any(|(a, e)| (a - e).abs() > 1e-9)
                });
        assert!(
            !mismatch,
            "ray bundle deviates from the recorded reference.\n\
             columns: {LABELS:?}\n\
             current values:\n{}",
            format_snapshot(actual)
        );
    }

    /// Format a snapshot as a Rust array literal that can be pasted into a test.
    ///
    /// # Arguments
    ///
    /// * `snapshot` - the snapshot to format, see [`ray_bundle_snapshot`].
    ///
    /// # Returns
    ///
    /// A multi-line string containing one `[..]` row per ray. Digit group separators are inserted
    /// so that the result satisfies `clippy::unreadable_literal` when pasted into a test.
    #[must_use]
    pub fn format_snapshot(snapshot: &[[f64; SNAPSHOT_WIDTH]]) -> String {
        snapshot
            .iter()
            .map(|ray| {
                let values = ray
                    .iter()
                    .map(|value| group_fraction_digits(&format!("{value:.12}")))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("            [{values}],")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Insert `_` every three digits behind the decimal point of a formatted number.
    ///
    /// # Arguments
    ///
    /// * `formatted` - a decimal number that already contains a decimal point.
    ///
    /// # Returns
    ///
    /// The same number with grouped fraction digits, e.g. `1.234567` becomes `1.234_567`.
    fn group_fraction_digits(formatted: &str) -> String {
        let Some((integer_part, fraction)) = formatted.split_once('.') else {
            return formatted.to_owned();
        };
        let grouped = fraction
            .as_bytes()
            .chunks(3)
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<Vec<_>>()
            .join("_");
        format!("{integer_part}.{grouped}")
    }

    pub fn test_analyze_geometric_no_isometry<T: Default + AnalysisRayTrace>(
        input_port_name: &str,
    ) {
        let mut node = T::default();
        assert!(
            node.ports()
                .names(&PortType::Input)
                .contains(&(input_port_name.into())),
            "wrong input port name used"
        );
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert(input_port_name.into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default());
        assert!(output.is_err());
    }
}
