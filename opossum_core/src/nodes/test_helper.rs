#[cfg(test)]
pub mod test_helper {
    use crate::{
        analyzers::{
            Analyzable, RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        apertures::{ApertureShape, ApertureType, CircleShape, GaussianShape},
        core_optics::{NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, OpticRef, PortType},
        distributions::position::Hexapolar,
        error::{OpmResult, OpossumError},
        gain::{AMP_CONFIG, ConstGain, GainModel},
        geometry::body::{Body, CLEAR_APERTURE, default_clear_aperture},
        joule,
        light::{LightData, LightResult, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        prelude::Aperture,
        properties::Proptype,
        utils::{LockExt, geom_transformation::Isometry, test_helper::test_helper::check_logs},
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::{Point3, Vector3};
    use std::sync::{Arc, Mutex};
    use uom::si::{energy::joule, f64::Length, length::millimeter};
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
    /// Read the [`GainModel`] out of a node's `amp config` property.
    ///
    /// [`GainModel`] is [`Copy`], so this works just as well on a node held behind a lock guard as
    /// on an owned node - hence the single [`NodeAttr`] parameter instead of one accessor per
    /// container.
    ///
    /// # Arguments
    ///
    /// * `node_attr` - attributes of the node to inspect.
    ///
    /// # Returns
    ///
    /// The configured [`GainModel`].
    ///
    /// # Panics
    ///
    /// Panics if the node does not declare an `amp config` property or if that property holds a
    /// different [`Proptype`].
    pub fn amp_config_of(node_attr: &NodeAttr) -> GainModel {
        let Ok(Proptype::GainModel(model)) = node_attr.get_property(AMP_CONFIG) else {
            panic!(
                "node '{}' has no '{AMP_CONFIG}' property holding a gain model",
                node_attr.node_type()
            );
        };
        *model
    }

    /// Assert that a node with a volume declares an inactive `amp config` by default.
    ///
    /// Declaring the property unconditionally is what makes "turn this component into an
    /// amplifier" an ordinary property change; defaulting to [`GainModel::None`] is what keeps
    /// that declaration from altering any existing result.
    ///
    /// # Panics
    ///
    /// Panics if the property is missing or does not default to [`GainModel::None`].
    pub fn test_amp_config_default<T: Default + OpticNode>() {
        let node = T::default();
        assert_eq!(amp_config_of(node.node_attr()), GainModel::None);
        assert!(!amp_config_of(node.node_attr()).is_active());
    }

    /// Assert that a non-default `amp config` survives a serialization round trip.
    ///
    /// The round trip goes through [`OpticRef`], which is the very path an `.opm` file takes: the
    /// node type string is used to construct a fresh default node whose properties are then
    /// patched with the ones found in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be serialized or deserialized.
    ///
    /// # Panics
    ///
    /// Panics if the gain model is not preserved by the round trip.
    pub fn test_amp_config_serde_roundtrip<T: Default + Analyzable + 'static>() -> OpmResult<()> {
        let mut node = T::default();
        let model = GainModel::Const(ConstGain::new(3.0)?);
        node.node_attr_mut()
            .set_property(AMP_CONFIG, model.into())?;

        let optic_ref = OpticRef::new(Arc::new(Mutex::new(node)), None);
        let serialized =
            ron::to_string(&optic_ref).map_err(|e| OpossumError::Other(e.to_string()))?;
        let deserialized: OpticRef =
            ron::from_str(&serialized).map_err(|e| OpossumError::Other(e.to_string()))?;

        assert_eq!(
            amp_config_of(deserialized.optical_ref.lock_opm()?.node_attr()),
            model,
            "gain model was not preserved by the round trip"
        );
        Ok(())
    }

    /// Remove one property entry from a serialized node, key and value.
    ///
    /// Used to turn a freshly written node into what a file written before that property existed
    /// looks like. The entry is cut out by scanning for the first comma that is not nested inside
    /// the value (or for the end of the property map, if the entry is the last one), so it works no
    /// matter where in the map the property sits. It assumes the value contains no string literal
    /// with brackets or commas in it, which holds for every property this is used on.
    ///
    /// # Arguments
    ///
    /// * `serialized` - the serialized node.
    /// * `property_name` - name of the property to remove.
    ///
    /// # Returns
    ///
    /// The serialized node without that property.
    ///
    /// # Panics
    ///
    /// Panics if the serialized node does not contain the property at all, which would make the
    /// calling test vacuous.
    fn remove_property_entry(serialized: &str, property_name: &str) -> String {
        let entry_start = serialized
            .find(&format!("\"{property_name}\""))
            .unwrap_or_else(|| {
                panic!("serialized node does not contain the {property_name} property")
            });
        let mut depth = 0i32;
        let mut entry_end = serialized.len();
        for (offset, character) in serialized[entry_start..].char_indices() {
            match character {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' if depth == 0 => {
                    // The end of the enclosing property map: this was the last entry.
                    entry_end = entry_start + offset;
                    break;
                }
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    entry_end = entry_start + offset + 1;
                    break;
                }
                _ => {}
            }
        }
        format!("{}{}", &serialized[..entry_start], &serialized[entry_end..])
    }

    /// Assert that a file written before the `amp config` property existed still loads.
    ///
    /// Such a file simply has no entry for the property. Because `set_node_attr` merges the
    /// properties of the file into those of a freshly constructed default node, the default has to
    /// survive — otherwise every existing `.opm` file would break.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be serialized or deserialized.
    ///
    /// # Panics
    ///
    /// Panics if the property could not be removed from the serialized form (which would make the
    /// test vacuous) or if the loaded node does not fall back to [`GainModel::None`].
    pub fn test_amp_config_absent_in_file<T: Default + Analyzable + 'static>() -> OpmResult<()> {
        let deserialized = load_without_property::<T>(AMP_CONFIG)?;
        assert_eq!(
            amp_config_of(deserialized.optical_ref.lock_opm()?.node_attr()),
            GainModel::None,
            "loading a file without the property must fall back to the default"
        );
        Ok(())
    }

    /// Assert that a file written before the `clear aperture` property existed still loads.
    ///
    /// The counterpart of [`test_amp_config_absent_in_file`] for the transversal extent: such a
    /// file has to come out with the standard extent rather than with no property at all.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be serialized or deserialized.
    ///
    /// # Panics
    ///
    /// Panics if the loaded node does not fall back to [`default_clear_aperture`].
    pub fn test_clear_aperture_absent_in_file<T: Default + Analyzable + 'static>() -> OpmResult<()>
    {
        let deserialized = load_without_property::<T>(CLEAR_APERTURE)?;
        let clear_aperture = {
            let node = deserialized.optical_ref.lock_opm()?;
            let Ok(Proptype::Aperture(shape)) = node.node_attr().get_property(CLEAR_APERTURE)
            else {
                panic!("the loaded node has no '{CLEAR_APERTURE}' property holding a shape");
            };
            shape.clone()
        };
        assert_eq!(
            clear_aperture,
            default_clear_aperture(),
            "loading a file without the property must fall back to the default"
        );
        Ok(())
    }

    /// Serialize a default node, drop one property entry again and load the result.
    ///
    /// This emulates a file written before that property existed. Because `set_node_attr` merges
    /// the properties of the file into those of a freshly constructed default node, the default has
    /// to survive — otherwise every existing `.opm` file would break.
    ///
    /// # Arguments
    ///
    /// * `property_name` - name of the property to drop.
    ///
    /// # Returns
    ///
    /// The node loaded from the reduced serialization.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be serialized or deserialized.
    ///
    /// # Panics
    ///
    /// Panics if the property could not be removed from the serialized form, which would make the
    /// calling test vacuous.
    fn load_without_property<T: Default + Analyzable + 'static>(
        property_name: &str,
    ) -> OpmResult<OpticRef> {
        let optic_ref = OpticRef::new(Arc::new(Mutex::new(T::default())), None);
        let serialized =
            ron::to_string(&optic_ref).map_err(|e| OpossumError::Other(e.to_string()))?;
        let without_property = remove_property_entry(&serialized, property_name);
        assert!(
            !without_property.contains(property_name),
            "the {property_name} entry was not removed, the test would be vacuous"
        );
        ron::from_str(&without_property).map_err(|e| OpossumError::Other(e.to_string()))
    }

    /// Assert that the body of a volume node matches the geometry its properties describe.
    ///
    /// The body is not configured separately: it is derived from the very surfaces
    /// `update_surfaces()` places from the node's curvature and thickness properties. What ties the
    /// two together is the on-axis path length — it has to come out as exactly the node's center
    /// thickness, the same distance the entry surface → exit surface pass covers.
    ///
    /// The optical axis starts exactly on the entrance surface, so this also exercises the case of
    /// a ray originating on a bounding surface, which is how a refracted ray enters the volume.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed or if its body cannot be derived.
    ///
    /// # Panics
    ///
    /// Panics if the node has no `center thickness` property, if the optical axis does not pass
    /// through the volume, or if the derived geometry does not match the property.
    pub fn test_volume_body<T: Default + OpticNode>() -> OpmResult<()> {
        let mut node = T::default();
        node.set_isometry(Isometry::identity())?;
        let center_thickness = center_thickness_of(&node);
        let axis_ray = Ray::origin_along_z(nanometer!(1053.0), joule!(1.0))?;
        let axis_path_length = |node: &T| -> OpmResult<Length> {
            node.volume_body()?
                .path_length_inside(&axis_ray)?
                .ok_or_else(|| {
                    OpossumError::Other("the optical axis does not pass through the volume".into())
                })
        };
        assert_abs_diff_eq!(
            axis_path_length(&node)?.value,
            center_thickness.value,
            epsilon = 1e-12
        );
        let body = node.volume_body()?;
        let on_axis_point =
            |z_position: Length| Point3::new(millimeter!(0.0), millimeter!(0.0), z_position);
        assert!(body.contains(&on_axis_point(center_thickness * 0.5))?);
        assert!(!body.contains(&on_axis_point(millimeter!(-1.0)))?);
        assert!(!body.contains(&on_axis_point(center_thickness + millimeter!(1.0)))?);
        // Inverting a node reverses the direction light travels through it, not the geometry.
        node.set_inverted(true)?;
        assert_abs_diff_eq!(
            axis_path_length(&node)?.value,
            center_thickness.value,
            epsilon = 1e-12
        );
        Ok(())
    }

    /// Assert that the transversal extent of a volume node is its clear aperture, and nothing else.
    ///
    /// The clear aperture is what a supplier quotes as the size of the component, and a volume node
    /// starts out with the 25 mm standard. A port [`Aperture`] must not influence it: masking the
    /// light in front of a component does not make the component smaller.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed or if its body cannot be derived.
    ///
    /// # Panics
    ///
    /// Panics if the node has no `center thickness` property or if its extent does not follow the
    /// clear aperture.
    pub fn test_clear_aperture<T: Default + OpticNode>() -> OpmResult<()> {
        let mut node = T::default();
        node.set_isometry(Isometry::identity())?;
        let mid_thickness = center_thickness_of(&node) * 0.5;
        let point_at = |radius: Length| Point3::new(radius, millimeter!(0.0), mid_thickness);
        // the default extent is a circle of 12.5 mm radius
        assert!(node.volume_body()?.contains(&point_at(millimeter!(12.4)))?);
        assert!(!node.volume_body()?.contains(&point_at(millimeter!(12.6)))?);
        // a port aperture masks the light, it does not resize the medium
        node.set_aperture(
            &PortType::Input,
            "input_1",
            &Aperture::new_circle(millimeter!(1.0), ApertureType::Hole, None)?,
        )?;
        assert!(node.volume_body()?.contains(&point_at(millimeter!(12.4)))?);
        // a wider clear aperture does
        node.node_attr_mut().set_property(
            CLEAR_APERTURE,
            ApertureShape::BinaryCircle(CircleShape::new(millimeter!(25.0))?).into(),
        )?;
        assert!(node.volume_body()?.contains(&point_at(millimeter!(24.9)))?);
        assert!(!node.volume_body()?.contains(&point_at(millimeter!(25.1)))?);
        // a shape that does not state where the medium ends leaves the volume undefined. An open
        // aperture is one of them: two curved surfaces may happen to close the volume on their own,
        // but nothing guarantees that they do.
        for undefined_extent in [
            ApertureShape::Open,
            ApertureShape::Gaussian(GaussianShape::new((millimeter!(5.0), millimeter!(5.0)))?),
        ] {
            node.node_attr_mut()
                .set_property(CLEAR_APERTURE, undefined_extent.into())?;
            assert!(node.volume_body().is_err());
        }
        Ok(())
    }

    /// Read the `center thickness` property of a volume node.
    ///
    /// # Arguments
    ///
    /// * `node` - the volume node to inspect.
    ///
    /// # Returns
    ///
    /// The center thickness of the node.
    ///
    /// # Panics
    ///
    /// Panics if the node does not declare a `center thickness` property.
    fn center_thickness_of<T: OpticNode>(node: &T) -> Length {
        let Ok(Proptype::Length(center_thickness)) =
            node.node_attr().get_property("center thickness")
        else {
            panic!(
                "node '{}' has no 'center thickness' property",
                node.node_attr().node_type()
            );
        };
        *center_thickness
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

    /// Assert that a volume node propagates [`volume_regression_rays`] to the recorded reference.
    ///
    /// Every node that encloses a volume pins its entry surface → volume → exit surface behaviour
    /// down with the same three rays; only the node and the expected numbers differ. Keeping the
    /// scaffolding here means a change to the port names or to the ray bundle is made once instead
    /// of once per node type.
    ///
    /// # Arguments
    ///
    /// * `node` - the volume node under test, already placed via `set_isometry`.
    /// * `expected` - the recorded reference snapshot, see [`ray_bundle_snapshot`].
    ///
    /// # Errors
    ///
    /// Returns an error if the regression ray bundle cannot be built or if the analysis fails.
    ///
    /// # Panics
    ///
    /// Panics if the node yields no geometric ray data on its output port, or if any captured
    /// value deviates from `expected`.
    pub fn test_volume_propagation_regression<T: AnalysisRayTrace>(
        node: &mut T,
        expected: &[[f64; SNAPSHOT_WIDTH]],
    ) -> OpmResult<()> {
        let mut incoming_data = LightResult::default();
        incoming_data.insert(
            "input_1".into(),
            LightData::Geometric(volume_regression_rays()?),
        );
        let output = AnalysisRayTrace::analyze(node, incoming_data, &RayTraceConfig::default())?;
        let Some(LightData::Geometric(rays)) = output.get("output_1") else {
            panic!("expected geometric ray data at the output port");
        };
        assert_ray_bundle_snapshot(&ray_bundle_snapshot(rays), expected);
        Ok(())
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
