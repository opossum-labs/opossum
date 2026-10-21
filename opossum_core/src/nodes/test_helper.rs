#[cfg(test)]
pub mod helper {
    use crate::{
        analyzers::{
            Analyzable, GhostFocusConfig, RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            ghostfocus::AnalysisGhostFocus,
            raytrace::AnalysisRayTrace,
        },
        apertures::{CircleShape, OpticShape},
        coatings::{CoatingConstantR, CoatingType},
        core_optics::{
            NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, OpticPorts, OpticRef, PortType,
            Volumetric, node_attr::NodePositioning, optic_surface::OpticSurface,
        },
        error::{OpmResult, OpossumError},
        geometry::{
            Face, Geometry,
            body::{Body, OPTIC_SHAPE, default_optic_shape},
        },
        joule,
        light::{LightData, LightRays, LightResult, Ray, Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::{EnergyMeter, NodeGroup, SourcePort, ThinMirror},
        percent,
        properties::Proptype,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::{Point2, Point3, Vector3};
    use uom::si::{energy::joule, f64::Length, length::millimeter};
    use uuid::Uuid;
    /// Assert that a node can be inverted and that reverting the inversion restores its ports.
    ///
    /// The surfaces of the inverted node are rebuilt in between, as loading a file, positioning and
    /// alignment do: that must leave the node's stored ports physical.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be inverted or its surfaces cannot be built.
    ///
    /// # Panics
    ///
    /// Panics if the node does not report being inverted, if its stored ports change or if
    /// reverting the inversion does not restore its ports.
    pub fn test_inverted<T: Default + OpticNode>() -> OpmResult<()> {
        let names = |ports: &OpticPorts| {
            (
                ports.names(&PortType::Input),
                ports.names(&PortType::Output),
            )
        };
        let mut node = T::default();
        let upright = names(&node.ports());
        let stored = names(node.node_attr().raw_ports());
        node.set_inverted(true)?;
        assert_eq!(node.inverted(), true);
        node.update_surfaces()?;
        assert_eq!(
            names(node.node_attr().raw_ports()),
            stored,
            "the stored ports of an inverted node must stay physical"
        );
        node.set_inverted(false)?;
        assert_eq!(
            names(&node.ports()),
            upright,
            "reverting the inversion must restore the ports"
        );
        Ok(())
    }
    /// Assert that a node reflects completely on `input_1` and `output_1`: the ports and the
    /// surfaces built from them carry a constant reflectivity of 100 %.
    ///
    /// # Panics
    ///
    /// Panics if a port or its surface carries another coating.
    pub fn test_reflects_completely<T: Default + OpticNode>() {
        let node = T::default();
        let full: CoatingType = CoatingConstantR::new(percent!(100.0)).unwrap().into();
        let ports = node.ports();
        for (port_type, name) in [(PortType::Input, "input_1"), (PortType::Output, "output_1")] {
            assert_eq!(ports.coating(&port_type, name), Some(&full), "port {name}");
            assert_eq!(
                node.get_optic_surface(name).map(OpticSurface::coating),
                Some(&full),
                "surface {name}"
            );
        }
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
    /// Assert that a ray inside a component's optic shape passes it and one just outside does
    /// not: a ray trace loses it and warns about the loss, a ghost focus analysis lets it run past
    /// unchanged.
    ///
    /// The node is placed at the origin and bounded by the round default optic shape of 12.5 mm
    /// ([`default_optic_shape`]), whatever its own default. Both rays travel along z, 12.4 mm and
    /// 12.6 mm off the axis, just inside and just outside of it. Whether a missed ray is lost or
    /// runs past is the analyzer's missed surface strategy (`Stop` for a ray trace, `Ignore` for a
    /// ghost focus analysis).
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed, bounded or analyzed.
    pub fn test_rays_beyond_optic_shape_are_lost<
        T: Default + AnalysisRayTrace + AnalysisGhostFocus,
    >() -> OpmResult<()> {
        let make = || -> OpmResult<T> {
            let mut node = placed_at_origin::<T>()?;
            node.set_property(OPTIC_SHAPE, default_optic_shape().into())?;
            node.update_surfaces()?;
            Ok(node)
        };
        let (inside, outside) = around_the_default_rim();
        let rays = || rays_along_z_from(&[inside, outside], nanometer!(1000.0));
        let mut node = make()?;
        let warning = format!(
            "Rays were lost at '{}' ({}): they ran outside its optic shape or fell below the energy threshold. Results might not be accurate.",
            node.node_attr().name(),
            node.node_attr().node_type()
        );
        testing_logger::setup();
        let traced = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::from([("input_1".into(), LightData::Geometric(rays()?))]),
            &RayTraceConfig::default(),
        )?;
        testing_logger::validate(|logs| {
            assert!(
                logs.iter().any(|log| log.body == warning),
                "a ray trace warns that it lost the ray outside: {:?}",
                logs.iter().map(|log| &log.body).collect::<Vec<_>>()
            );
        });
        let Some(LightData::Geometric(traced)) = traced.get("output_1") else {
            panic!("expected ray data at the output port");
        };
        let valid: Vec<bool> = traced.iter().map(Ray::valid).collect();
        assert_eq!(
            valid,
            [true, false],
            "a ray trace keeps the ray inside, loses the one outside"
        );
        let passed = AnalysisGhostFocus::analyze(
            &mut make()?,
            LightRays::from([("input_1".into(), vec![rays()?])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let Some(beside) = passed
            .get("output_1")
            .and_then(|bundles| bundles.first())
            .and_then(|bundle| bundle.iter().nth(1))
        else {
            panic!("expected the ray outside the optic shape at the output port");
        };
        assert!(
            beside.valid(),
            "a ghost focus analysis lets the ray outside run past"
        );
        assert_eq!(beside.position(), outside);
        assert_eq!(beside.direction(), Vector3::z());
        assert_eq!(beside.energy(), joule!(1.0));
        Ok(())
    }
    /// Assert that a reflecting component reflects a ray inside its optic shape and loses one
    /// just outside, in a ray trace and in a ghost focus analysis alike.
    ///
    /// Both rays travel along z, 12.4 mm and 12.6 mm off the axis, around the default optic shape
    /// of 12.5 mm. A ray that misses a mirror or a grating has no reflected counterpart, whatever the
    /// analyzer's missed surface strategy. At 500 nm the default grating diffracts a ray at normal
    /// incidence.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed or analyzed.
    ///
    /// # Panics
    ///
    /// Panics if an analysis puts out no rays or another set of them.
    pub fn test_reflections_beyond_optic_shape_are_lost<
        T: Default + AnalysisRayTrace + AnalysisGhostFocus,
    >() -> OpmResult<()> {
        let (inside, outside) = around_the_default_rim();
        let rays = || rays_along_z_from(&[inside, outside], nanometer!(500.0));
        let traced = AnalysisRayTrace::analyze(
            &mut placed_at_origin::<T>()?,
            LightResult::from([("input_1".into(), LightData::Geometric(rays()?))]),
            &RayTraceConfig::default(),
        )?;
        let Some(LightData::Geometric(traced)) = traced.get("output_1") else {
            panic!("expected ray data at the output port");
        };
        let ghosts = AnalysisGhostFocus::analyze(
            &mut placed_at_origin::<T>()?,
            LightRays::from([("input_1".into(), vec![rays()?])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let Some(ghosts) = ghosts.get("output_1").and_then(|bundles| bundles.first()) else {
            panic!("expected a ray bundle at the output port");
        };
        for (analysis, reflected) in [("ray trace", traced), ("ghost focus analysis", ghosts)] {
            let heights: Vec<f64> = reflected
                .iter()
                .filter(|ray| ray.valid())
                .map(|ray| ray.position().y.get::<millimeter>())
                .collect();
            assert_eq!(
                heights.len(),
                1,
                "a {analysis} reflects the ray inside only: {heights:?}"
            );
            assert_abs_diff_eq!(heights[0], inside.y.get::<millimeter>(), epsilon = 1e-9);
        }
        Ok(())
    }
    /// Assert that a detector records only the light within its optic shape but lets every ray
    /// pass, in a ray trace and in a ghost focus analysis alike.
    ///
    /// Two rays travel along z, 5 mm and 20 mm off the axis. Within its default window, a square of
    /// 25 mm, a detector records the one inside only and warns that the other was not recorded.
    /// Set open, it records both. Either way both rays leave it valid, with their direction and
    /// energy.
    ///
    /// # Arguments
    ///
    /// * `recorded` - reads the light the detector recorded.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed, opened or analyzed.
    ///
    /// # Panics
    ///
    /// Panics if the detector holds back a ray, records other rays than expected or does not warn.
    pub fn test_detector_records_inside_and_passes_all<
        T: Default + AnalysisRayTrace + AnalysisGhostFocus,
    >(
        recorded: impl Fn(&T) -> Option<&LightData>,
    ) -> OpmResult<()> {
        let starts = [millimeter!(0.0, 5.0, -10.0), millimeter!(0.0, 20.0, -10.0)];
        let rays = || rays_along_z_from(&starts, nanometer!(1000.0));
        let heights = |rays: &Rays| -> Vec<f64> {
            rays.iter()
                .filter(|ray| ray.valid())
                .map(|ray| ray.position().y.get::<millimeter>())
                .collect()
        };
        let assert_passed = |passed: &Rays, analysis: &str| {
            assert_valid_energies(
                passed,
                &[1.0, 1.0],
                &format!("a {analysis} passes all rays"),
            );
            assert!(passed.iter().all(|ray| ray.direction() == Vector3::z()));
        };
        for bounded in [false, true] {
            let mut detector = placed_at_origin::<T>()?;
            if !bounded {
                detector.set_property(OPTIC_SHAPE, OpticShape::Open.into())?;
                detector.update_surfaces()?;
            }
            let expected: &[f64] = if bounded { &[5.0] } else { &[5.0, 20.0] };
            let context = if bounded { "bounded" } else { "unbounded" };
            let assert_recorded = |recorded: &[f64], analysis: &str| {
                assert_eq!(
                    recorded.len(),
                    expected.len(),
                    "{context}, {analysis}: recorded {recorded:?}"
                );
                for (height, expected) in recorded.iter().zip(expected) {
                    assert_abs_diff_eq!(height, expected, epsilon = 1e-9);
                }
            };

            testing_logger::setup();
            let output = AnalysisRayTrace::analyze(
                &mut detector,
                LightResult::from([("input_1".into(), LightData::Geometric(rays()?))]),
                &RayTraceConfig::default(),
            )?;
            let Some(LightData::Geometric(passed)) = output.get("output_1") else {
                panic!("expected ray data at the output port");
            };
            assert_passed(passed, "ray trace");
            let Some(LightData::Geometric(traced)) = recorded(&detector) else {
                panic!("the detector recorded no rays in a ray trace");
            };
            assert_recorded(&heights(traced), "ray trace");
            let hit_points = detector
                .get_optic_surface("input_1")
                .and_then(|surface| surface.hit_map().get_first_hitpoints())
                .map_or(0, |points| points.len());
            assert_eq!(hit_points, expected.len(), "{context}: hit points");
            testing_logger::validate(|logs| {
                let warned = logs
                    .iter()
                    .any(|log| log.body.contains("were not recorded"));
                assert_eq!(warned, bounded, "{context}: warning about unrecorded rays");
            });

            let output = AnalysisGhostFocus::analyze(
                &mut detector,
                LightRays::from([("input_1".into(), vec![rays()?])]),
                &GhostFocusConfig::default(),
                &mut Vec::new(),
                0,
            )?;
            let Some(passed) = output.get("output_1").and_then(|bundles| bundles.first()) else {
                panic!("expected a ray bundle at the output port");
            };
            assert_passed(passed, "ghost focus analysis");
            let Some(LightData::GhostFocus(ghosts)) = recorded(&detector) else {
                panic!("the detector recorded no rays in a ghost focus analysis");
            };
            let ghost_heights: Vec<f64> = ghosts.iter().flat_map(heights).collect();
            assert_recorded(&ghost_heights, "ghost focus analysis");
        }
        Ok(())
    }
    /// Build a source, an absolutely placed flat mirror 100 mm in front of it and an energy meter
    /// 50 mm back on the reflected beam. The mirror's optic shape is then shrunk to a radius of
    /// 5 mm the way the backend changes a property (see
    /// [`scenery_with_a_mirror_changed_behind_its_back`]).
    ///
    /// # Returns
    ///
    /// The scenery, the id of the source and the id of the energy meter.
    ///
    /// # Errors
    ///
    /// Returns an error if the scenery cannot be built.
    pub fn scenery_with_a_mirror_shrunk_behind_its_back() -> OpmResult<(NodeGroup, Uuid, Uuid)> {
        let shrunk: OpticShape = CircleShape::new(millimeter!(5.0))?.into();
        scenery_with_a_mirror_changed_behind_its_back(OPTIC_SHAPE, shrunk.into())
    }
    /// Build a source, an absolutely placed flat mirror 100 mm in front of it and an energy meter
    /// 50 mm back on the reflected beam. A property of the mirror is then changed the way the
    /// backend changes it: written into the node's attributes, without the node rebuilding its
    /// surfaces.
    ///
    /// # Arguments
    ///
    /// * `name` - the name of the mirror's property to change.
    /// * `value` - its new value.
    ///
    /// # Returns
    ///
    /// The scenery, the id of the source and the id of the energy meter.
    ///
    /// # Errors
    ///
    /// Returns an error if the scenery cannot be built or the property cannot be set.
    pub fn scenery_with_a_mirror_changed_behind_its_back(
        name: &str,
        value: Proptype,
    ) -> OpmResult<(NodeGroup, Uuid, Uuid)> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::new_along_z(
            millimeter!(100.0),
        )?))?;
        let mirror = scenery.add_node(mirror)?;
        let meter = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(100.0))?;
        scenery.connect_nodes(mirror, "output_1", meter, "input_1", millimeter!(50.0))?;
        scenery.with_node_attr_mut(mirror, |node_attr| node_attr.set_property(name, value))??;
        Ok((scenery, source, meter))
    }
    /// Return the number of valid rays the energy meter with the given id recorded.
    ///
    /// # Errors
    ///
    /// Returns an error if the scenery has no such node.
    ///
    /// # Panics
    ///
    /// Panics if the node is no energy meter or recorded no rays.
    pub fn rays_recorded_by_meter(scenery: &NodeGroup, meter: Uuid) -> OpmResult<usize> {
        let node = scenery.node(meter)?;
        let Some(meter) = node.as_any().downcast_ref::<EnergyMeter>() else {
            panic!("the node is no energy meter");
        };
        match meter.light_data() {
            Some(LightData::Geometric(rays)) => Ok(rays.nr_of_rays(true)),
            Some(LightData::GhostFocus(bundles)) => {
                Ok(bundles.iter().map(|rays| rays.nr_of_rays(true)).sum())
            }
            _ => panic!("the energy meter recorded no rays"),
        }
    }
    /// Return the start points of two rays, 12.4 mm and 12.6 mm off the axis: just inside and
    /// just outside the default optic shape of 12.5 mm, 10 mm in front of a node at the origin.
    fn around_the_default_rim() -> (Point3<Length>, Point3<Length>) {
        (millimeter!(0.0, 12.4, -10.0), millimeter!(0.0, 12.6, -10.0))
    }
    /// Return a bundle of rays along z, one from each start point, carrying 1 J each.
    ///
    /// # Errors
    ///
    /// Returns an error if the wavelength is not positive and finite.
    pub fn rays_along_z_from(starts: &[Point3<Length>], wavelength: Length) -> OpmResult<Rays> {
        let rays = starts
            .iter()
            .map(|start| Ray::new_collimated(*start, wavelength, joule!(1.0)))
            .collect::<OpmResult<Vec<_>>>()?;
        Ok(Rays::from(rays))
    }
    /// Return a default node of the given type, placed at the origin.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed.
    pub fn placed_at_origin<T: Default + OpticNode>() -> OpmResult<T> {
        let mut node = T::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        Ok(node)
    }
    /// Assert the energies of the valid rays of a bundle, in order.
    ///
    /// # Arguments
    ///
    /// * `rays` - the bundle to check.
    /// * `expected` - the energy of each valid ray, in J.
    /// * `context` - what the bundle is, for the failure message.
    ///
    /// # Panics
    ///
    /// Panics if the number of valid rays or one of their energies differs.
    pub fn assert_valid_energies(rays: &Rays, expected: &[f64], context: &str) {
        let energies: Vec<f64> = rays
            .iter()
            .filter(|ray| ray.valid())
            .map(|ray| ray.energy().get::<joule>())
            .collect();
        assert_eq!(energies.len(), expected.len(), "{context}: {energies:?}");
        for (energy, expected) in energies.iter().zip(expected) {
            assert_abs_diff_eq!(energy, expected, epsilon = 1e-12);
        }
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

    /// Assert that a file written before the `optic shape` property existed still loads.
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
    /// Panics if the loaded node does not fall back to the optic shape a default node of its
    /// type has.
    pub fn test_optic_shape_absent_in_file<T: Default + Analyzable + 'static>() -> OpmResult<()> {
        let shape_of = |node_attr: &NodeAttr| {
            let Ok(Proptype::OpticShape(optic_shape)) = node_attr.get_property(OPTIC_SHAPE) else {
                panic!("the node has no '{OPTIC_SHAPE}' property holding an optic shape");
            };
            optic_shape.clone()
        };
        let deserialized = load_without_property::<T>(OPTIC_SHAPE)?;
        assert_eq!(
            shape_of(deserialized.node_attr()),
            shape_of(T::default().node_attr()),
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
        let optic_ref = OpticRef::new(Box::new(T::default()));
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
    /// The body is not configured separately: it is derived from the node's geometry, the same
    /// description of its curvature and thickness properties that `update_surfaces()` installs the
    /// traced surfaces from. What ties the two together is the on-axis path length — it has to
    /// come out as exactly the node's center thickness, the same distance the entry surface → exit
    /// surface pass covers.
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
    pub fn test_volume_body<T: Default + Volumetric>() -> OpmResult<()> {
        let mut node = T::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let center_thickness = center_thickness_of(&node);
        let on_axis = millimeter!(0.0, 0.0, 0.0);
        assert_abs_diff_eq!(
            path_length_through(&node, on_axis)?.value,
            center_thickness.value,
            epsilon = 1e-12
        );
        let geometry = placed_geometry(&node)?;
        let body = body_of(&geometry)?;
        let on_axis_point =
            |z_position: Length| Point3::new(millimeter!(0.0), millimeter!(0.0), z_position);
        assert!(body.contains(&on_axis_point(center_thickness * 0.5)));
        assert!(!body.contains(&on_axis_point(millimeter!(-1.0))));
        assert!(!body.contains(&on_axis_point(center_thickness + millimeter!(1.0))));
        // Inverting a node reverses the direction light travels through it, not the geometry.
        node.set_inverted(true)?;
        assert_abs_diff_eq!(
            path_length_through(&node, on_axis)?.value,
            center_thickness.value,
            epsilon = 1e-12
        );
        Ok(())
    }

    /// Trace a collimated ray from the given position through the volume of a node.
    ///
    /// # Arguments
    ///
    /// * `node` - the volume node to trace through.
    /// * `position` - where the ray starts, in global coordinates.
    ///
    /// # Returns
    ///
    /// The geometrical path length of the ray inside the node's volume.
    ///
    /// # Errors
    ///
    /// Returns an error if the body cannot be derived or if the ray does not pass through it.
    pub fn path_length_through<T: Volumetric>(
        node: &T,
        position: Point3<Length>,
    ) -> OpmResult<Length> {
        let ray = Ray::new_collimated(position, nanometer!(1053.0), joule!(1.0))?;
        body_of(&placed_geometry(node)?)?
            .path_length_inside(&ray)
            .ok_or_else(|| {
                OpossumError::Other(format!(
                    "a ray at {position:?} does not pass through the volume of node '{}'",
                    node.name()
                ))
            })
    }

    /// Return the geometry of a node, moved to where the node sits.
    ///
    /// The geometry states the faces in their physical order, so an inverted node encloses the
    /// same volume as an upright one.
    ///
    /// # Errors
    ///
    /// Returns an error if the node's geometry cannot be derived or if the node has none.
    pub fn placed_geometry<T: OpticNode + ?Sized>(node: &T) -> OpmResult<Geometry> {
        let geometry = node.geometry()?.ok_or_else(|| {
            OpossumError::Other(format!("node '{}' has no geometry", node.name()))
        })?;
        Ok(geometry.moved(&node.effective_node_iso().unwrap_or_else(Isometry::identity)))
    }

    /// Return the volume a geometry encloses.
    ///
    /// # Errors
    ///
    /// Returns an error if the geometry is a single surface, which encloses no volume.
    pub fn body_of(geometry: &Geometry) -> OpmResult<&dyn Body> {
        geometry
            .kind()
            .as_body()
            .ok_or_else(|| OpossumError::Other("a single surface encloses no volume".into()))
    }

    /// Assert that the transversal extent of a volume node is its optic shape, and nothing else.
    ///
    /// The optic shape is what a supplier quotes as the size of the component, and a volume node
    /// starts out with a default of its type.
    ///
    /// # Arguments
    ///
    /// * `default_edge` - how far the default optic shape of the node type reaches along x.
    ///
    /// # Errors
    ///
    /// Returns an error if the node cannot be placed or if its body cannot be derived.
    ///
    /// # Panics
    ///
    /// Panics if the node has no `center thickness` property or if its extent does not follow the
    /// optic shape.
    pub fn test_optic_shape<T: Default + Volumetric>(default_edge: Length) -> OpmResult<()> {
        let mut node = T::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mid_thickness = center_thickness_of(&node) * 0.5;
        let point_at = |radius: Length| Point3::new(radius, millimeter!(0.0), mid_thickness);
        let geometry = placed_geometry(&node)?;
        let body = body_of(&geometry)?;
        assert!(body.contains(&point_at(default_edge - millimeter!(0.1))));
        assert!(!body.contains(&point_at(default_edge + millimeter!(0.1))));
        // a wider optic shape widens it
        node.node_attr_mut().set_property(
            OPTIC_SHAPE,
            OpticShape::from(CircleShape::new(millimeter!(25.0))?).into(),
        )?;
        let geometry = placed_geometry(&node)?;
        let body = body_of(&geometry)?;
        assert!(body.contains(&point_at(millimeter!(24.9))));
        assert!(!body.contains(&point_at(millimeter!(25.1))));
        // An open optic shape leaves the volume undefined and is rejected where it is set: two
        // curved surfaces may happen to close the volume on their own, but nothing guarantees it.
        assert!(
            node.node_attr_mut()
                .set_property(OPTIC_SHAPE, OpticShape::Open.into())
                .is_err()
        );
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

    /// Describe every runtime surface a node has installed, one line of text per surface.
    ///
    /// Input surfaces come first, then output surfaces, each sorted by port name. A line states
    /// the port, the kind of [`GeoSurface`](crate::geometry::GeoSurface), the anchor
    /// relative to the node (translation in millimeter, then its local x and z axis), the
    /// surface's own `local_z_at` at (0, 0), (5 mm, 0) and (0, 5 mm) in millimeter - which pins
    /// radius, sign and orientation of a curved surface - and the index of the first surface
    /// sharing the same geometric surface. Values are rounded to 1e-6, so the order in which
    /// isometries are composed cannot show up.
    ///
    /// # Arguments
    ///
    /// * `node` - the node whose surfaces are described.
    ///
    /// # Returns
    ///
    /// One line per installed surface.
    #[must_use]
    pub fn surface_snapshot<T: OpticNode + ?Sized>(node: &T) -> Vec<String> {
        let runtime = node.node_attr().runtime_surfaces();
        let surfaces = runtime
            .inputs
            .iter()
            .map(|(name, surface)| ("in", name, surface))
            .chain(
                runtime
                    .outputs
                    .iter()
                    .map(|(name, surface)| ("out", name, surface)),
            )
            .map(|(port_type, name, surface)| (port_type, name.as_str(), surface.surface().clone()))
            .collect::<Vec<_>>();
        describe_surfaces(node, &surfaces)
    }

    /// Describe the surfaces a node's [`Geometry`](crate::geometry::Geometry) builds, in the
    /// format of [`surface_snapshot`] and under the port names the node has installed: the
    /// entrance surface under every input port, the exit surface under every output port.
    ///
    /// Comparing the two snapshots shows whether the geometry describes exactly the surfaces the
    /// node installs.
    ///
    /// # Arguments
    ///
    /// * `node` - the node whose geometry is described.
    ///
    /// # Returns
    ///
    /// One line per port with an installed surface; no line at all for a node without geometry.
    ///
    /// # Errors
    ///
    /// Returns an error if the geometry cannot be derived, or if no face of it lies where an
    /// installed surface does.
    pub fn geometry_snapshot<T: OpticNode + ?Sized>(node: &T) -> OpmResult<Vec<String>> {
        let Some(geometry) = node.geometry()? else {
            return Ok(Vec::new());
        };
        let placed = geometry.moved(&node.effective_node_iso().unwrap_or_else(Isometry::identity));
        let kind = placed.kind();
        let runtime = node.node_attr().runtime_surfaces();
        let mut surfaces = Vec::new();
        for (port_type, ports) in [("in", &runtime.inputs), ("out", &runtime.outputs)] {
            for (name, installed) in ports {
                // Which face a port sits at is the node's business: find the face that lies where
                // the installed surface does.
                let face = (0..kind.face_count())
                    .filter_map(|index| kind.face(index))
                    .find(|face| *face == installed.surface())
                    .ok_or_else(|| {
                        OpossumError::Other(format!(
                            "no face of the geometry lies where the surface at port '{name}' does"
                        ))
                    })?;
                surfaces.push((port_type, name.as_str(), face.clone()));
            }
        }
        Ok(describe_surfaces(node, &surfaces))
    }

    /// Describe surfaces one line each, as documented at [`surface_snapshot`].
    ///
    /// # Arguments
    ///
    /// * `node` - the node the surfaces belong to.
    /// * `surfaces` - port type, port name and placed face of each surface.
    fn describe_surfaces<T: OpticNode + ?Sized>(
        node: &T,
        surfaces: &[(&str, &str, Face)],
    ) -> Vec<String> {
        let node_iso = node.effective_node_iso().unwrap_or_else(Isometry::identity);
        let mut lines = Vec::with_capacity(surfaces.len());
        for (port_type, port_name, face) in surfaces {
            let shared = surfaces
                .iter()
                .position(|(_, _, other)| other == face)
                .expect("a surface is shared with itself");
            // The anchor: where the face sits relative to the node.
            let anchor = Isometry::new_from_transform(
                node_iso.get_inv_transform() * face.frame().get_transform(),
            );
            let translation = anchor.translation();
            let geo = face.surface();
            let sag = [(0.0, 0.0), (5.0, 0.0), (0.0, 5.0)]
                .map(|(x, y)| {
                    geo.local_z_at(&Point2::new(millimeter!(x), millimeter!(y)))
                        .map_or_else(|| "none".to_owned(), |z| rounded(z.get::<millimeter>()))
                })
                .join(", ");
            let name = geo.name();
            lines.push(format!(
                "{port_type} {port_name}: {name} t=({}, {}, {}) x={} z={} sag=({sag}) shared={shared}",
                rounded(translation.x.get::<millimeter>()),
                rounded(translation.y.get::<millimeter>()),
                rounded(translation.z.get::<millimeter>()),
                rounded_vector(&anchor.transform_vector_f64(&Vector3::x())),
                rounded_vector(&anchor.transform_vector_f64(&Vector3::z())),
            ));
        }
        lines
    }

    /// Compare labelled snapshots against the ones recorded when the behaviour was last accepted.
    ///
    /// Both tables are compared as a whole, so a failure reports every deviating entry at once.
    ///
    /// # Arguments
    ///
    /// * `actual` - pairs of label and snapshot (see [`surface_snapshot`] or [`body_snapshot`])
    ///   from the current run, sorted by label.
    /// * `expected` - the recorded reference, sorted by label.
    ///
    /// # Panics
    ///
    /// Panics if the tables differ. The panic message contains the current table as a paste-ready
    /// literal, so an intentional change of behaviour can be re-baselined directly.
    pub fn assert_snapshots(actual: &[(&str, Vec<String>)], expected: &[(&str, &[&str])]) {
        let matches = actual.len() == expected.len()
            && actual.iter().zip(expected).all(
                |((actual_label, actual_lines), (expected_label, expected_lines))| {
                    actual_label == expected_label
                        && actual_lines
                            .iter()
                            .map(String::as_str)
                            .eq(expected_lines.iter().copied())
                },
            );
        let table = actual
            .iter()
            .map(|(label, lines)| {
                let lines = lines
                    .iter()
                    .map(|line| format!("\n                    \"{line}\","))
                    .collect::<String>();
                format!("            (\n                \"{label}\",\n                &[{lines}\n                ],\n            ),")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            matches,
            "snapshots deviate from the recorded reference.\ncurrent values:\n{table}"
        );
    }

    /// Describe everything a gain or absorption model reads from a volume body.
    ///
    /// One line each for the body's frame (translation in millimeter, local x and z axis) and its
    /// bounding box, one line per chord of a set of straight and oblique rays through it, and one
    /// line marking which points of a grid lie inside it (`#`) or not (`.`). Rays and points are
    /// stated in the node's frame, so the same component gives the same chords wherever it is
    /// placed. Values are rounded as by [`surface_snapshot`].
    ///
    /// # Arguments
    ///
    /// * `body` - the body to describe.
    /// * `node_frame` - the placement of the node the body belongs to.
    ///
    /// # Returns
    ///
    /// The lines describing the body.
    ///
    /// # Errors
    ///
    /// Returns an error if the body cannot answer one of the questions.
    pub fn body_snapshot(body: &dyn Body, node_frame: &Isometry) -> OpmResult<Vec<String>> {
        let frame = body.isometry();
        let translation = frame.translation();
        let range = |range: std::ops::Range<Length>| {
            format!(
                "[{}, {}]",
                rounded(range.start.get::<millimeter>()),
                rounded(range.end.get::<millimeter>())
            )
        };
        // A body whose extent cannot be determined is recorded as such rather than aborting.
        let bounds = body.bounding_box().map_or_else(
            |error| format!("box: {error}"),
            |bounds| {
                format!(
                    "box x={} y={} z={}",
                    range(bounds.x_range()),
                    range(bounds.y_range()),
                    range(bounds.z_range())
                )
            },
        );
        let mut lines = vec![
            format!(
                "frame t=({}, {}, {}) x={} z={}",
                rounded(translation.x.get::<millimeter>()),
                rounded(translation.y.get::<millimeter>()),
                rounded(translation.z.get::<millimeter>()),
                rounded_vector(&frame.transform_vector_f64(&Vector3::x())),
                rounded_vector(&frame.transform_vector_f64(&Vector3::z())),
            ),
            bounds,
        ];
        for (x, y) in [
            (0.0, 0.0),
            (5.0, 0.0),
            (0.0, 5.0),
            (-8.0, 3.0),
            (12.4, 0.0),
            (12.6, 0.0),
        ] {
            for tilt in [0.0_f64, 10.0] {
                let chord = body
                    .path_length_inside(&probe_ray(x, y, tilt, node_frame)?)
                    .map_or_else(|| "none".to_owned(), |l| rounded(l.get::<millimeter>()));
                lines.push(format!("chord from ({x}, {y}) at {tilt} deg: {chord}"));
            }
        }
        let mut inside = String::new();
        for x in [-12.6, -12.4, 0.0, 6.0, 12.4, 12.6] {
            for y in [0.0, 7.0] {
                for z in [-0.5, 0.5, 2.5, 4.5, 5.5, 9.5, 10.5] {
                    let point = node_frame.transform_point(&millimeter!(x, y, z));
                    inside.push(if body.contains(&point) { '#' } else { '.' });
                }
            }
        }
        lines.push(format!("inside: {inside}"));
        Ok(lines)
    }

    /// Describe where rays meet every surface a node has installed.
    ///
    /// One line per surface and ray of a fixed set of straight and oblique rays: the hit point
    /// and the surface normal there, or `miss`, and whether the surface records the hit. Rays,
    /// points and normals are stated in the node's frame and rounded as by [`surface_snapshot`].
    /// Unlike that snapshot, nothing here depends on how a surface or its placement is
    /// represented.
    ///
    /// # Arguments
    ///
    /// * `node` - the node whose installed surfaces are hit.
    ///
    /// # Returns
    ///
    /// The lines describing the hits, ordered by port type and port name.
    ///
    /// # Errors
    ///
    /// Returns an error if a ray cannot be built.
    pub fn hit_snapshot<T: OpticNode + ?Sized>(node: &T) -> OpmResult<Vec<String>> {
        let node_frame = node.effective_node_iso().unwrap_or_else(Isometry::identity);
        let runtime = node.node_attr().runtime_surfaces();
        let mut lines = Vec::new();
        for (port_type, ports) in [("in", &runtime.inputs), ("out", &runtime.outputs)] {
            for (name, surface) in ports {
                for (x, y, tilt) in [
                    (0.0, 0.0, 0.0),
                    (5.0, 0.0, 10.0),
                    (-8.0, 3.0, 0.0),
                    (0.0, 5.0, 10.0),
                    (12.4, 0.0, 0.0),
                    (12.6, 0.0, 0.0),
                ] {
                    let hit = surface
                        .intersect(&probe_ray(x, y, tilt, &node_frame)?)
                        .map_or_else(
                            || "miss".to_owned(),
                            |hit| {
                                let local = node_frame.inverse_transform_point(&hit.point());
                                let recorded = if hit.recorded() {
                                    "recorded"
                                } else {
                                    "not recorded"
                                };
                                format!(
                                    "({}, {}, {}) n={} {recorded}",
                                    rounded(local.x.get::<millimeter>()),
                                    rounded(local.y.get::<millimeter>()),
                                    rounded(local.z.get::<millimeter>()),
                                    rounded_vector(
                                        &node_frame.inverse_transform_vector_f64(&hit.normal())
                                    ),
                                )
                            },
                        );
                    lines.push(format!(
                        "{port_type} {name} from ({x}, {y}) at {tilt} deg: {hit}"
                    ));
                }
            }
        }
        Ok(lines)
    }

    /// Build a ray that starts 50 mm in front of a node and runs along its axis, tilted towards
    /// the node's y axis.
    ///
    /// # Arguments
    ///
    /// * `x`, `y` - where the ray starts across the node's axis, in millimeter.
    /// * `tilt` - the angle between the ray and the node's axis, in degree.
    /// * `node_frame` - the placement of the node.
    ///
    /// # Returns
    ///
    /// The ray, in global coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error if the ray cannot be built.
    fn probe_ray(x: f64, y: f64, tilt: f64, node_frame: &Isometry) -> OpmResult<Ray> {
        let direction = Vector3::new(0.0, tilt.to_radians().sin(), tilt.to_radians().cos());
        Ok(Ray::new(
            Point3::new(millimeter!(x), millimeter!(y), millimeter!(-50.0)),
            direction,
            nanometer!(1053.0),
            joule!(1.0),
        )?
        .transformed_ray(node_frame))
    }

    /// Format a value rounded to six decimal places.
    ///
    /// # Arguments
    ///
    /// * `value` - the value to format.
    ///
    /// # Returns
    ///
    /// The rounded value; `-0.000000` is written as `0.000000`.
    fn rounded(value: f64) -> String {
        // Adding 0.0 turns a negative zero into a positive one.
        format!("{:.6}", (value * 1e6).round() / 1e6 + 0.0)
    }

    /// Format a vector with every component rounded as by [`rounded`].
    ///
    /// # Arguments
    ///
    /// * `vector` - the vector to format.
    ///
    /// # Returns
    ///
    /// The components as `(x, y, z)`.
    fn rounded_vector(vector: &Vector3<f64>) -> String {
        format!(
            "({}, {}, {})",
            rounded(vector.x),
            rounded(vector.y),
            rounded(vector.z)
        )
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
