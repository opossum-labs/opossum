#![warn(missing_docs)]

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

use crate::{
    analyzers::{AnalyzerType, propagation_strategy::MissedSurfaceStrategy},
    coatings::{CoatingType, SplittingConfig, SplittingConfigBuilder},
    core_optics::{
        NodeAttr, OpticNode, OpticNodeExt, PortType, SurfacePorts, node_attr::HasNodeAttr,
        surface_effect::Refraction,
    },
    error::{OpmResult, OpossumError},
    geometry::{
        Geometry, SurfaceGeometry,
        body::{OPTIC_SHAPE, default_optic_shape},
    },
    light::{
        LightData, Rays,
        rays::ReflectedPart,
        spectrum::{Spectrum, merge_spectra},
    },
    nodes::NodeRegistration,
    properties::validator::Validator,
};
use opm_macros_lib::OpmNode;

inventory::submit! {
    NodeRegistration::new::<BeamSplitter>("beam splitter", "ideal beam splitter")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightpink")]
/// An ideal beamsplitter node: a thin splitting surface in the plane of the node.
///
/// The coating of its `surface` is the split. The surface is a splitting surface (see
/// [`SurfacePorts::splitting`]), so it takes only a [`CoatingType::Separator`], by default one that
/// transmits half of the light and reflects the rest. In ray tracing, the transmitted light keeps
/// its direction while the reflected light is mirrored about the splitting surface. Hence, the beam splitter has to be tilted via its alignment (e.g. by 45°)
/// to send the reflected light sideways; an untilted beam splitter reflects it back onto the
/// incoming beam.
///
/// ## Optical Ports
///   - Inputs
///     - `input_1`
///     - `input_2`
///   - Outputs
///     - `out1_trans1_refl2`
///     - `out2_trans2_refl1`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `surface`: the coating (the split) and the LIDT of the splitting surface
///   - `optic shape`: the extent of the splitting surface; a ray beyond it misses the splitter
///     and follows the analyzer's missed surface strategy
pub struct BeamSplitter {
    node_attr: NodeAttr,
}

impl Default for BeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("beam splitter");
        node_attr
            .create_property_with_validator(
                OPTIC_SHAPE,
                "transversal extent of the component",
                Validator::OpticShapeIsBounded,
                default_optic_shape().into(),
            )
            .unwrap();
        let mut bs = Self { node_attr };
        bs.create_default_surface_properties().unwrap();
        bs.update_surfaces().unwrap();
        bs
    }
}
impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] whose surface splits by the given configuration, as a
    /// [`CoatingType::Separator`].
    ///
    /// ## Errors
    /// This function returns an error if the configuration does not transmit within `[0.0, 1.0]`.
    pub fn new(name: &str, config: &SplittingConfigBuilder) -> OpmResult<Self> {
        let mut bs = Self::default();
        bs.node_attr.set_name(name);
        bs.set_coating(SurfaceGeometry::FACE, &config.clone().into())?;
        Ok(bs)
    }
    /// Returns the splitting configuration of the separator coating of this [`BeamSplitter`]'s
    /// surface, by which the energy analysis splits.
    ///
    /// # Errors
    /// This function returns an error if the configuration cannot be built, or if the surface has
    /// no separator coating, which its property does not take.
    pub fn splitting_config(&self) -> OpmResult<SplittingConfig> {
        let surface = self.face_name(SurfaceGeometry::FACE)?;
        match self.surface_config(&surface)?.coating() {
            CoatingType::Separator(config) => config.build(),
            coating => Err(OpossumError::Analysis(format!(
                "the energy analysis splits only by a separator coating, but the {surface} of \
                 beam splitter '{}' has the coating '{coating}'",
                self.node_attr.name()
            ))),
        }
    }
    fn split_spectrum(
        input: Option<&LightData>,
        splitting_config: &SplittingConfig,
    ) -> OpmResult<(Option<Spectrum>, Option<Spectrum>)> {
        if let Some(in1) = input {
            match in1 {
                LightData::Energy(spectrum) => {
                    match splitting_config {
                        SplittingConfig::Ratio(r) => {
                            let mut s = spectrum.clone();
                            s.scale_vertical(r)?;
                            let out1_spectrum = Some(s);
                            let mut s = spectrum.clone();
                            s.scale_vertical(&(1.0 - r))?;
                            let out2_spectrum = Some(s);
                            Ok((out1_spectrum, out2_spectrum))
                        },
                        SplittingConfig::Spectrum(spec) => {
                            let mut s = spectrum.clone();
                            let split_spectrum=s.split_by_spectrum(spec);
                            let out1_spectrum = Some(s);
                            let out2_spectrum = Some(split_spectrum);
                            Ok((out1_spectrum, out2_spectrum))
                        },
                    }
                },
                _ => {
                    Err(OpossumError::Analysis(
                        "expected LightData::Energy value at input port. A reason might be that the wrong analzer was used for the given light source data type. Try to use another analyzer (e.g. ray tracing)".into(),
                    ))
                }
            }
        } else {
            Ok((None, None))
        }
    }
    fn analyze_energy(
        &self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        let splitting_config = self.splitting_config()?;
        let (out1_1_spectrum, out1_2_spectrum) = Self::split_spectrum(in1, &splitting_config)?;
        let (out2_1_spectrum, out2_2_spectrum) = Self::split_spectrum(in2, &splitting_config)?;

        let out1_spec = merge_spectra(out1_1_spectrum, out2_2_spectrum);
        let out2_spec = merge_spectra(out1_2_spectrum, out2_1_spectrum);
        let mut out1_data: Option<LightData> = None;
        let mut out2_data: Option<LightData> = None;
        if let Some(out1_spec) = out1_spec {
            out1_data = Some(LightData::Energy(out1_spec));
        }
        if let Some(out2_spec) = out2_spec {
            out2_data = Some(LightData::Energy(out2_spec));
        }
        Ok((out1_data, out2_data))
    }
    /// Processes rays arriving at a single input port.
    ///
    /// The rays are split by the coating of the splitting surface: the transmitted part keeps its
    /// direction, the reflected part is mirrored about the surface normal.
    ///
    /// # Arguments
    ///
    /// * `input` - the light arriving at the port, if any.
    /// * `port_name` - the name of the input port.
    /// * `missed_surface_strategy` - what happens to rays that do not hit the splitting surface.
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected rays, both empty if there is no input.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    ///   - the input is not [`LightData::Geometric`].
    ///   - the surface of the port cannot be found.
    ///   - the splitting of the rays fails.
    fn process_input_port(
        &mut self,
        input: Option<&LightData>,
        port_name: &str,
        missed_surface_strategy: MissedSurfaceStrategy,
    ) -> OpmResult<(Rays, Rays)> {
        let Some(light_data) = input else {
            return Ok((Rays::default(), Rays::default()));
        };
        let LightData::Geometric(rays) = light_data else {
            return Err(OpossumError::Analysis(format!(
                "Expected LightData::Geometric at port '{port_name}'"
            )));
        };
        let mut rays = rays.clone();
        let face = self.face_of_port(port_name)?;
        let Some(geometry) = self.node_attr_mut().runtime_geometry_mut() else {
            return Err(OpossumError::OpticPort(format!(
                "Input optic surface not found for port '{port_name}'"
            )));
        };
        let reflected = geometry.interact_with_rays(
            face,
            &mut rays,
            &Refraction(None),
            ReflectedPart::Branch,
            missed_surface_strategy,
        )?;
        Ok((rays, reflected))
    }
    fn analyze_raytrace(
        &mut self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        }

        let in1_port_name = &self.ports().names(&PortType::Input)[0];
        let in2_port_name = &self.ports().names(&PortType::Input)[1];

        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy(_) => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => ray_trace_config.missed_surface_strategy(),
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };

        // Process both inputs
        let (mut main_rays1, split_rays1) =
            self.process_input_port(in1, in1_port_name, *missed_surface_strategy)?;
        let (mut main_rays2, split_rays2) =
            self.process_input_port(in2, in2_port_name, *missed_surface_strategy)?;

        // Merge the transmitted and reflected rays for the two outputs.
        main_rays1.merge(&split_rays2); // out1 = trans1 + refl2
        main_rays2.merge(&split_rays1); // out2 = trans2 + refl1
        if let AnalyzerType::RayTrace(config) = analyzer_type {
            main_rays1.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            main_rays2.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        }

        Ok((
            Some(LightData::Geometric(main_rays1)),
            Some(LightData::Geometric(main_rays2)),
        ))
    }
}
impl OpticNode for BeamSplitter {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(self.cross_section()?)))
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts {
            inputs: &[
                ("input_1", SurfaceGeometry::FACE),
                ("input_2", SurfaceGeometry::FACE),
            ],
            outputs: &[
                ("out1_trans1_refl2", SurfaceGeometry::FACE),
                ("out2_trans2_refl1", SurfaceGeometry::FACE),
            ],
            splitting: &[SurfaceGeometry::FACE],
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            GhostFocusConfig, RayTraceConfig, ghostfocus::AnalysisGhostFocus,
            raytrace::AnalysisRayTrace,
        },
        coatings::{CoatingConstantR, CoatingType},
        core_optics::{NodeAttrExt, PortType},
        light::{LightRays, LightResult},
        millimeter, nanometer,
        nodes::test_helper::helper::*,
        percent,
    };
    #[test]
    fn default() -> OpmResult<()> {
        let node = BeamSplitter::default();
        assert_eq!(
            node.surface_config(&node.face_name(SurfaceGeometry::FACE)?)?
                .coating(),
            &CoatingType::Separator(SplittingConfigBuilder::FixedRatio(0.5))
        );
        assert!(
            node.node_attr()
                .get_property("splitter config builder")
                .is_err()
        );
        assert_eq!(node.name(), "beam splitter");
        assert_eq!(node.node_type(), "beam splitter");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "lightpink");
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let splitter = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        assert_eq!(splitter.name(), "test");
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(-0.01)).is_err());
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(1.01)).is_err());
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<BeamSplitter>()
    }
    #[test]
    fn ports() {
        let node = BeamSplitter::default();
        let mut input_ports = node.ports().names(&PortType::Input);
        input_ports.sort();
        assert_eq!(input_ports, vec!["input_1", "input_2"]);
        let mut output_ports = node.ports().names(&PortType::Output);
        output_ports.sort();
        assert_eq!(output_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut node = BeamSplitter::default();
        node.set_inverted(true)?;
        let mut input_ports = node.ports().names(&PortType::Input);
        input_ports.sort();
        assert_eq!(input_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
        let mut output_ports = node.ports().names(&PortType::Output);
        output_ports.sort();
        assert_eq!(output_ports, vec!["input_1", "input_2"]);
        Ok(())
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<BeamSplitter>()
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<BeamSplitter>()
    }
    /// Rays along z towards a splitter at the origin, one at each of the given heights in mm.
    fn rays_at(heights: &[f64]) -> OpmResult<Rays> {
        let starts: Vec<_> = heights
            .iter()
            .map(|y| millimeter!(0.0, *y, -10.0))
            .collect();
        rays_along_z_from(&starts, nanometer!(1000.0))
    }
    /// Send rays into `input_1` of the splitter in a ray trace.
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected bundle.
    fn ray_trace(splitter: &mut BeamSplitter, rays: Rays) -> OpmResult<(Rays, Rays)> {
        let output = AnalysisRayTrace::analyze(
            splitter,
            LightResult::from([("input_1".into(), LightData::Geometric(rays))]),
            &RayTraceConfig::default(),
        )?;
        let (Some(LightData::Geometric(transmitted)), Some(LightData::Geometric(reflected))) = (
            output.get("out1_trans1_refl2"),
            output.get("out2_trans2_refl1"),
        ) else {
            panic!("expected ray data at both outputs");
        };
        Ok((transmitted.clone(), reflected.clone()))
    }
    #[test]
    fn the_split_follows_the_surface_coating() -> OpmResult<()> {
        let mut splitter = placed_at_origin::<BeamSplitter>()?;
        splitter.set_coating(
            SurfaceGeometry::FACE,
            &SplittingConfigBuilder::FixedRatio(0.3).into(),
        )?;
        let (transmitted, reflected) = ray_trace(&mut splitter, rays_at(&[0.0])?)?;
        assert_valid_energies(&transmitted, &[0.3], "the separator transmits 30 %");
        assert_valid_energies(&reflected, &[0.7], "the separator reflects the rest");
        Ok(())
    }
    /// Both parts of the light leave the splitting surface, so it takes only a separator.
    #[test]
    fn a_beam_splitter_takes_only_a_separator_coating() -> OpmResult<()> {
        let mut splitter = BeamSplitter::default();
        let surface = splitter.face_name(SurfaceGeometry::FACE)?;
        for coating in [
            CoatingType::IdealAR,
            CoatingConstantR::new(percent!(30.0))?.into(),
            CoatingType::Fresnel,
        ] {
            assert!(
                splitter
                    .set_coating(SurfaceGeometry::FACE, &coating)
                    .is_err(),
                "{coating}"
            );
        }
        assert_eq!(
            splitter.surface_config(&surface)?.coating(),
            &CoatingType::Separator(SplittingConfigBuilder::default())
        );
        Ok(())
    }
    /// Send rays into `input_1` of the splitter in a ghost focus analysis.
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected bundle.
    fn ghost_trace(splitter: &mut BeamSplitter, rays: Rays) -> OpmResult<(Rays, Rays)> {
        let output = AnalysisGhostFocus::analyze(
            splitter,
            LightRays::from([("input_1".into(), vec![rays])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let bundle = |port: &str| {
            output
                .get(port)
                .and_then(|bundles| bundles.first())
                .cloned()
                .unwrap_or_else(|| panic!("expected a ray bundle at port {port}"))
        };
        Ok((bundle("out1_trans1_refl2"), bundle("out2_trans2_refl1")))
    }
    /// A ray beyond the optic shape misses the splitter, whatever the analyzer does with it: a
    /// ray trace loses it, a ghost focus analysis lets it run on, unchanged. Only a ray that hits
    /// is split.
    #[test]
    fn rays_beyond_optic_shape_are_lost() -> OpmResult<()> {
        let (transmitted, reflected) = ray_trace(
            &mut placed_at_origin::<BeamSplitter>()?,
            rays_at(&[12.4, 12.6])?,
        )?;
        assert_valid_energies(
            &transmitted,
            &[0.5],
            "a ray trace transmits half of the ray inside",
        );
        assert_valid_energies(
            &reflected,
            &[0.5],
            "a ray trace reflects half of the ray inside",
        );

        let (transmitted, reflected) = ghost_trace(
            &mut placed_at_origin::<BeamSplitter>()?,
            rays_at(&[12.4, 12.6])?,
        )?;
        assert_valid_energies(
            &transmitted,
            &[0.5, 1.0],
            "a ghost focus analysis lets the ray outside run on",
        );
        assert_valid_energies(&reflected, &[0.5], "only the ray inside is reflected");
        let beside = transmitted.iter().nth(1).expect("the ray outside");
        assert_eq!(beside.position(), millimeter!(0.0, 12.6, -10.0));
        Ok(())
    }
}
