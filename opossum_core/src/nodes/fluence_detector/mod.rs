#![warn(missing_docs)]
//! Fluence measurement node
pub mod fluence_data;

use crate::{
    analyzers::{
        AnalyzerKind, energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, SurfacePorts,
        hit_map::fluence_estimator::FluenceEstimator,
        observable::{MetricDimension, ObservableDescriptor},
        optic_node::APERTURE_LIMIT_NOTE,
        optic_surface::OpticSurface,
    },
    error::OpmResult,
    geometry::{
        Geometry, SurfaceShape,
        body::{OPTIC_SHAPE, default_square_optic_shape},
    },
    light::LightData,
    nodes::NodeRegistration,
    properties::{Properties, Proptype, validator::Validator},
    reporting::node_report::{NodeReport, NodeReportResult},
};
use fluence_data::FluenceData;
use log::warn;
use opm_macros_lib::OpmNode;

/// Alias for uom `RadiantExposure`, as this name is rather uncommon to use for laser scientists.
pub type Fluence = uom::si::f64::RadiantExposure;

inventory::submit! {
    NodeRegistration::new::<FluenceDetector>("fluence detector", "fluence detector")
}

/// A fluence monitor node
///
/// Calculates the spatial energy distribution (fluence) of an incoming ray bundle.
/// The algorithm used for calculating the fluence map is specified by the property `fluence estimator`.
/// By default, the Voronoi estimator is used ([`FluenceEstimator::Voronoi`]).
///
/// ## Optical Ports
///   - Inputs: `input_1`
///   - Outputs: `output_1`
///
/// ## Properties
///   - `name`
///   - `fluence estimator`
///   - `optic shape`: The window the light is recorded within (25 mm square by default).
#[derive(OpmNode, Clone, Debug)]
#[opm_node("hotpink")]
pub struct FluenceDetector {
    node_attr: NodeAttr,
    apodization_warning: bool,
    light_data: Option<LightData>,
}

impl Default for FluenceDetector {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("fluence detector");
        node_attr
            .create_property(
                "fluence estimator",
                "fluence estimator strategy",
                FluenceEstimator::Voronoi.into(),
            )
            .expect("Hardcoded property creation must not fail");
        node_attr
            .create_property_with_validator(
                OPTIC_SHAPE,
                "window the light is recorded within",
                Validator::OpticShape {
                    may_be_open: true,
                    bored: false,
                },
                default_square_optic_shape().into(),
            )
            .expect("Hardcoded property creation must not fail");
        let mut fld = Self {
            node_attr,
            apodization_warning: false,
            light_data: None,
        };
        fld.create_default_surface_properties()
            .expect("Hardcoded surface properties creation must not fail");
        fld.update_surfaces()
            .expect("Updating surfaces on default fluence detector must not fail");
        fld
    }
}

impl FluenceDetector {
    /// Creates a new [`FluenceDetector`].
    #[must_use]
    pub fn new(name: &str) -> Self {
        let mut fld = Self::default();
        fld.node_attr.set_name(name);
        fld
    }

    /// Calculates the current [`FluenceData`] based on the hit map of the detector surface.
    ///
    /// Evaluates the hit map recorded during ray tracing using the currently configured [`FluenceEstimator`].
    #[must_use]
    pub fn calc_fluence_data(&self) -> Option<FluenceData> {
        let hit_map = self
            .get_optic_surface("input_1")
            .map(OpticSurface::hit_map)
            .filter(|hit_map| !hit_map.is_empty())?;
        let estimator = self.estimator()?;
        hit_map.calc_fluence_map((95, 83), &estimator).ok()
    }
    /// Returns the estimator this detector computes its fluence map with, Voronoi if its property
    /// holds none, or `None` if it has no such property.
    fn estimator(&self) -> Option<FluenceEstimator> {
        match self.node_attr.get_property("fluence estimator").ok()? {
            Proptype::FluenceEstimator(est) => Some(*est),
            _ => Some(FluenceEstimator::Voronoi),
        }
    }
}

impl OpticNode for FluenceDetector {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::detector(
            SurfaceShape::Plane,
            self.cross_section()?,
        )?))
    }

    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }

    fn needs_ray_tubes(&self) -> bool {
        self.estimator()
            .is_some_and(FluenceEstimator::needs_ray_tubes)
    }

    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
    }

    fn reset_data(&mut self) {
        self.set_light_data(None);
        self.apodization_warning = false;
        self.reset_optic_surfaces();
    }

    fn node_report(&self, uuid: &str, analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        if analyzer == AnalyzerKind::Energy {
            return Ok(NodeReportResult::incompatible_warning(format!(
                "Node '{}' ({}): A fluence map can only be calculated during a ray tracing or ghostfocus analysis.",
                self.name(),
                self.node_type()
            )));
        }

        let mut props = Properties::default();

        if let Some(fluence_data) = self.calc_fluence_data() {
            // Einheitliche kanonische Eigenschaftsnamen
            props.create(
                "Fluence",
                "2D spatial energy distribution",
                fluence_data.clone().into(),
            )?;
            props.create(
                "Peak Fluence",
                "Peak fluence of the distribution",
                Proptype::Fluence(fluence_data.peak()),
            )?;
            props.create(
                "Total energy",
                "Total energy of the distribution",
                Proptype::Energy(fluence_data.total_energy()),
            )?;
            // Explizite Angabe der verwendeten Schätzmethode
            props.create(
                "Estimator",
                "Fluence estimation algorithm used",
                (*fluence_data.estimator()).into(),
            )?;

            if self.apodization_warning {
                props.create(
                    "Warning",
                    "warning during analysis",
                    APERTURE_LIMIT_NOTE.into(),
                )?;
            }
        } else {
            warn!(
                "Could not calculate fluence map; surface hit map is empty or calculation failed."
            );
            props.create(
                "Warning",
                "warning during analysis",
                "Could not calculate the fluence map with the defined estimator. Please try another one.".into(),
            )?;
        }

        Ok(NodeReportResult::Report(NodeReport::new(
            self.node_type(),
            self.name(),
            uuid,
            props,
        )))
    }

    fn set_light_data(&mut self, ld: Option<LightData>) {
        self.light_data = ld;
    }

    fn observable_metrics(&self, analyzer: AnalyzerKind) -> Vec<ObservableDescriptor> {
        match analyzer {
            AnalyzerKind::RayTrace | AnalyzerKind::GhostFocus => vec![
                ObservableDescriptor::new(
                    "peak fluence",
                    "Peak fluence of the beam profile (J/m²)",
                    MetricDimension::Fluence,
                )
                .with_default_target(0.0),
                ObservableDescriptor::new(
                    "total energy",
                    "Total integrated energy across the fluence distribution (J)",
                    MetricDimension::Energy,
                )
                .with_default_target(0.0),
            ],
            AnalyzerKind::Energy => Vec::new(),
        }
    }

    fn read_observable(&self, metric_name: &str) -> Option<f64> {
        let fluence_data = self.calc_fluence_data()?;
        let lower = metric_name.to_lowercase();
        match lower.as_str() {
            // SI-Basiseinheit: J / m² (1 J/cm² = 10.000 J/m²)
            "peak fluence" | "peak" | "peak_fluence" => Some(fluence_data.peak().value),
            // SI-Basiseinheit: Joule (J)
            "total energy" | "energy" | "total_energy" => Some(fluence_data.total_energy().value),
            _ => None,
        }
    }
}

impl AnalysisGhostFocus for FluenceDetector {}
impl AnalysisEnergy for FluenceDetector {}
impl AnalysisRayTrace for FluenceDetector {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::EnergyConfig,
        core_optics::PortType,
        light::{LightData, LightResult, spectrum_helper::create_he_ne_spec},
        nanometer,
        nodes::test_helper::helper::*,
    };
    #[test]
    fn default() {
        let node = FluenceDetector::default();
        assert_eq!(node.name(), "fluence detector");
        assert_eq!(node.node_type(), "fluence detector");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "hotpink");
    }
    #[test]
    fn new() {
        let meter = FluenceDetector::new("test");
        assert_eq!(meter.name(), "test");
    }
    #[test]
    fn ports() {
        let meter = FluenceDetector::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut meter = FluenceDetector::default();
        meter.set_inverted(true)?;
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<FluenceDetector>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<FluenceDetector>()
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("wrong".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut node = FluenceDetector::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<FluenceDetector>()
    }
    #[test]
    fn records_inside_and_passes_all() -> OpmResult<()> {
        test_detector_records_inside_and_passes_all::<FluenceDetector>(|detector| {
            detector.light_data.as_ref()
        })
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = FluenceDetector::default();
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());

        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }
    #[test]
    fn observable_metrics_fluence_detector() {
        let detector = FluenceDetector::default();
        assert!(detector.observable_metrics(AnalyzerKind::Energy).is_empty());

        let metrics = detector.observable_metrics(AnalyzerKind::RayTrace);
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].name, "peak fluence");
        assert_eq!(metrics[0].dimension, MetricDimension::Fluence);
        assert_eq!(metrics[1].name, "total energy");
        assert_eq!(metrics[1].dimension, MetricDimension::Energy);
    }

    #[test]
    fn report_contains_estimator_property() -> OpmResult<()> {
        use crate::{
            analyzers::RayTraceConfig, distributions::position::Hexapolar, joule, light::Rays,
            millimeter, nanometer,
        };

        let mut detector = FluenceDetector::default();
        detector.set_positioning(crate::core_optics::node_attr::NodePositioning::Absolute(
            crate::utils::geom_transformation::Isometry::identity(),
        ))?;

        // 19 rays in 2 hexapolar rings, spanning the ray tubes a source spans for a Voronoi map
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), 2)?,
        )?;
        rays.attach_mesh()?;

        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Geometric(rays));

        let _ = AnalysisRayTrace::analyze(&mut detector, input, &RayTraceConfig::default())?;

        let NodeReportResult::Report(report) =
            detector.node_report("test_uuid", AnalyzerKind::RayTrace)?
        else {
            panic!("Expected report");
        };

        let props = report.properties();
        assert!(props.contains("Fluence"));
        assert!(props.contains("Peak Fluence"));
        assert!(props.contains("Total energy"));
        assert!(props.contains("Estimator"));

        // Verify read_observable retrieves raw SI values
        let peak = detector.read_observable("peak fluence");
        assert!(peak.is_some());
        assert!(peak.unwrap() > 0.0);

        let energy = detector.read_observable("total energy");
        assert!(energy.is_some());
        // Integrated fluence energy is a numerical surface integral across the discrete grid;
        // with few rays (19), edge truncation occurs, so check reasonable bounds instead of exact match.
        let e = energy.unwrap();
        assert!(
            e > 0.5 && e <= 1.05,
            "Integrated energy {e} should be reasonably close to 1.0 J"
        );

        assert_eq!(detector.read_observable("unknown"), None);
        Ok(())
    }

    /// The radius of the flat-top beam of [`map_of_a_flat_top`], in mm.
    const BEAM_RADIUS: f64 = 10.0;
    /// The number of hexapolar rings of the beam of [`map_of_a_flat_top`].
    const BEAM_RINGS: u8 = 15;
    /// The fluence of the flat-top beam of [`map_of_a_flat_top`], in J/cm²: 1 J spread evenly
    /// over its rays, each ring standing for an annulus one ring spacing wide, so the outermost
    /// ring reaches half a spacing beyond the beam radius.
    fn flat_top_fluence() -> f64 {
        let reach = (BEAM_RADIUS + BEAM_RADIUS / f64::from(BEAM_RINGS) / 2.0) / 10.0;
        1.0 / (std::f64::consts::PI * reach * reach)
    }
    /// Traces a collimated flat-top beam of 1 J at the given laser lines into a fluence detector
    /// 150 mm from the source and returns the Voronoi map the detector computes. With
    /// `bored_mirror`, a default bored mirror 100 mm from the source reflects the beam back onto
    /// the detector 50 mm in front of it.
    fn map_of_a_flat_top(
        lines: Vec<(uom::si::f64::Length, f64)>,
        bored_mirror: bool,
    ) -> OpmResult<FluenceData> {
        use crate::{
            analyzers::{Analyzer, RayTraceConfig, raytrace::RayTracingAnalyzer},
            distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
            joule, millimeter,
            nodes::{BoredMirror, NodeGroup, SourcePort},
            prelude::{CollimatedSrc, RayDataSource},
        };
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let detector = scenery.add_node(FluenceDetector::default())?;
        if bored_mirror {
            let mirror = scenery.add_node(BoredMirror::default())?;
            scenery.connect_nodes(source, "output_1", mirror, "mirror", millimeter!(100.0))?;
            scenery.connect_nodes(mirror, "in_front", detector, "input_1", millimeter!(50.0))?;
        } else {
            scenery.connect_nodes(source, "output_1", detector, "input_1", millimeter!(150.0))?;
        }
        let mut config = RayTraceConfig::default();
        config.map_source(
            source,
            RayDataSource::Collimated(CollimatedSrc::new(
                Hexapolar::new(millimeter!(BEAM_RADIUS), BEAM_RINGS)?.into(),
                UniformDist::new(joule!(1.0))?.into(),
                LaserLines::new(lines)?.into(),
            ))
            .into(),
        );
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        scenery
            .node(detector)?
            .as_any()
            .downcast_ref::<FluenceDetector>()
            .and_then(FluenceDetector::calc_fluence_data)
            .ok_or_else(|| crate::error::OpossumError::Other("the detector has no map".into()))
    }
    /// The value of the given map nearest to the given point of the detector, in J/cm².
    fn fluence_near(map: &FluenceData, x_mm: f64, y_mm: f64) -> OpmResult<f64> {
        use uom::si::{length::millimeter, radiant_exposure::joule_per_square_centimeter};
        let (x, y, values) = map.get_fluence_distribution()?;
        let nearest = |axis: &nalgebra::DVector<uom::si::f64::Length>, at: f64| {
            axis.iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (a.get::<millimeter>() - at)
                        .abs()
                        .total_cmp(&(b.get::<millimeter>() - at).abs())
                })
                .map_or(0, |(index, _)| index)
        };
        Ok(values[(nearest(&y, y_mm), nearest(&x, x_mm))].get::<joule_per_square_centimeter>())
    }
    /// A flat-top beam shows its fluence across the map.
    #[test]
    fn a_flat_top_beam_shows_its_fluence() -> OpmResult<()> {
        let map = map_of_a_flat_top(vec![(nanometer!(1000.0), 1.0)], false)?;
        for (x, y) in [(5.0, 0.0), (0.0, 5.0), (-3.0, -3.0), (7.0, -4.0)] {
            approx::assert_relative_eq!(
                fluence_near(&map, x, y)?,
                flat_top_fluence(),
                max_relative = 0.03
            );
        }
        Ok(())
    }
    /// Two laser lines that run the same way draw the same map as one line of the same energy.
    #[test]
    fn two_lines_running_alike_draw_the_map_of_one() -> OpmResult<()> {
        let one = map_of_a_flat_top(vec![(nanometer!(1000.0), 1.0)], false)?;
        let two = map_of_a_flat_top(
            vec![(nanometer!(1000.0), 1.0), (nanometer!(1064.0), 1.0)],
            false,
        )?;
        assert_eq!(one.shape(), two.shape());
        for (a, b) in one
            .interp_distribution()
            .iter()
            .zip(two.interp_distribution())
        {
            if !(a.is_nan() && b.is_nan()) {
                approx::assert_relative_eq!(a.value, b.value, max_relative = 1e-9, epsilon = 1e-9);
            }
        }
        Ok(())
    }
    /// The light a bored mirror reflects is a ring: the map stays dark in its hole and shows the
    /// full fluence right next to the edge of the bore.
    #[test]
    fn the_hole_behind_a_bored_mirror_stays_dark() -> OpmResult<()> {
        let map = map_of_a_flat_top(vec![(nanometer!(1000.0), 1.0)], true)?;
        let (center, next_to_the_bore, further_out) = (
            fluence_near(&map, 0.0, 0.0)?,
            fluence_near(&map, 3.0, 0.0)?,
            fluence_near(&map, 0.0, -6.0)?,
        );
        assert!(
            center < 0.01 * flat_top_fluence(),
            "the hole shows {center} J/cm²"
        );
        approx::assert_relative_eq!(next_to_the_bore, flat_top_fluence(), max_relative = 0.05);
        approx::assert_relative_eq!(further_out, flat_top_fluence(), max_relative = 0.03);
        Ok(())
    }
}
