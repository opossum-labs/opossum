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
        hit_map::{HitMap, fluence_estimator::FluenceEstimator},
        observable::{MetricDimension, ObservableDescriptor},
        optic_node::APERTURE_LIMIT_NOTE,
        optic_surface::OpticSurface,
    },
    error::OpmResult,
    geometry::{
        Geometry, SurfaceShape,
        body::{OPTIC_SHAPE, default_square_optic_shape},
    },
    joule,
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

/// The property that switches the error estimates of the report on (see the handbook, "Fluence
/// detector").
const ERROR_ESTIMATES: &str = "error estimates";

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
            .create_property(
                ERROR_ESTIMATES,
                "report how far the fluence may deviate from the map",
                false.into(),
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
    /// With the property `error estimates` switched on, a map that follows ray tubes carries the
    /// estimated deviation of the fluence from it (see [`HitMap::estimate_deviation`]).
    ///
    /// # Returns
    ///
    /// The map, or `None` if no light reached the detector.
    ///
    /// # Errors
    ///
    /// This function returns an error if the estimator cannot compute the map, e.g. a Voronoi map
    /// of rays that span no ray tubes.
    pub fn calc_fluence_data(&self) -> OpmResult<Option<FluenceData>> {
        let Some(hit_map) = self.recorded_hits() else {
            return Ok(None);
        };
        let Some(estimator) = self.estimator() else {
            return Ok(None);
        };
        let map = hit_map.calc_fluence_map((101, 101), &estimator)?;
        if self.reports_errors() && estimator.needs_ray_tubes() {
            return hit_map.estimate_deviation(map).map(Some);
        }
        Ok(Some(map))
    }
    /// Returns whether this detector reports how far the fluence may deviate from its map: its
    /// property `error estimates`.
    fn reports_errors(&self) -> bool {
        matches!(
            self.node_attr.get_property(ERROR_ESTIMATES),
            Ok(Proptype::Bool(true))
        )
    }
    /// Returns the hits this detector recorded, `None` if no light reached it.
    fn recorded_hits(&self) -> Option<&HitMap> {
        self.get_optic_surface("input_1")
            .map(OpticSurface::hit_map)
            .filter(|hit_map| !hit_map.is_empty())
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

        let map = self.calc_fluence_data();
        if let Ok(Some(fluence_data)) = map {
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
            let energy = self
                .recorded_hits()
                .map_or_else(|| joule!(0.0), HitMap::energy);
            props.create(
                "Total energy",
                "Energy of the light that hit the detector",
                Proptype::Energy(energy),
            )?;
            // Explizite Angabe der verwendeten Schätzmethode
            props.create(
                "Estimator",
                "Fluence estimation algorithm used",
                (*fluence_data.estimator()).into(),
            )?;
            if self.reports_errors() {
                if let (Some(uncertainty), Some(peak_uncertainty)) = (
                    fluence_data.uncertainty_map(),
                    fluence_data.peak_uncertainty(),
                ) {
                    props.create(
                        "Peak fluence uncertainty",
                        "How far the peak of the fluence may lie above the peak of the map, estimated from the curvature of the fluence between the rays",
                        Proptype::Fluence(peak_uncertainty),
                    )?;
                    props.create(
                        "Fluence uncertainty",
                        "Estimated size of the deviation of the fluence from the map at each point, from the curvature of the fluence between the rays",
                        uncertainty.into(),
                    )?;
                }
                if let Some(drawn) = fluence_data.drawn_energy()
                    && energy.value > 0.0
                {
                    props.create(
                        "Energy deviation of the map",
                        "Share by which the energy the map holds misses the energy that hit the detector",
                        Proptype::F64((drawn / energy).value - 1.0),
                    )?;
                }
                if let Some(share) = fluence_data.edge_share() {
                    props.create(
                        "Energy in edge margins",
                        "Share of the energy at the edges of the light, placed only to within half a ray tube",
                        Proptype::F64(share),
                    )?;
                }
            }

            if self.apodization_warning {
                props.create(
                    "Warning",
                    "warning during analysis",
                    APERTURE_LIMIT_NOTE.into(),
                )?;
            }
        } else {
            let reason = match map {
                Err(error) => format!("Could not calculate the fluence map: {error}"),
                Ok(_) => "No light reached the detector, so it shows no fluence map.".into(),
            };
            warn!("Node '{}' ({}): {reason}", self.name(), self.node_type());
            props.create("Warning", "warning during analysis", reason.into())?;
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
                    "Energy of the light that hit the detector (J)",
                    MetricDimension::Energy,
                )
                .with_default_target(0.0),
            ],
            AnalyzerKind::Energy => Vec::new(),
        }
    }

    fn read_observable(&self, metric_name: &str) -> Option<f64> {
        let lower = metric_name.to_lowercase();
        match lower.as_str() {
            // SI-Basiseinheit: J / m² (1 J/cm² = 10.000 J/m²)
            "peak fluence" | "peak" | "peak_fluence" => {
                Some(self.calc_fluence_data().ok().flatten()?.peak().value)
            }
            // SI-Basiseinheit: Joule (J)
            "total energy" | "energy" | "total_energy" => {
                Some(self.recorded_hits()?.energy().value)
            }
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
        utils::test_helper::helper::fluence_near,
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

    /// A detector that no light reached says so in its report, rather than blaming its estimator.
    #[test]
    fn the_report_says_when_no_light_arrived() -> OpmResult<()> {
        let NodeReportResult::Report(report) =
            FluenceDetector::default().node_report("test_uuid", AnalyzerKind::RayTrace)?
        else {
            panic!("Expected report");
        };
        let Proptype::String(warning) = report.properties().get("Warning")? else {
            panic!("Expected a warning");
        };
        assert!(warning.contains("No light reached"), "{warning}");
        Ok(())
    }
    /// A map that cannot be computed says why in the report: rays that span no ray tubes leave a
    /// Voronoi detector without a map.
    #[test]
    fn the_report_says_why_the_map_failed() -> OpmResult<()> {
        use crate::{
            analyzers::RayTraceConfig, distributions::position::Hexapolar, joule, light::Rays,
            millimeter,
        };
        let mut detector = FluenceDetector::default();
        detector.set_positioning(crate::core_optics::node_attr::NodePositioning::Absolute(
            crate::utils::geom_transformation::Isometry::identity(),
        ))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), 2)?,
        )?;
        let input = LightResult::from([("input_1".into(), LightData::Geometric(rays))]);
        AnalysisRayTrace::analyze(&mut detector, input, &RayTraceConfig::default())?;
        let NodeReportResult::Report(report) =
            detector.node_report("test_uuid", AnalyzerKind::RayTrace)?
        else {
            panic!("Expected report");
        };
        let Proptype::String(warning) = report.properties().get("Warning")? else {
            panic!("Expected a warning");
        };
        assert!(warning.contains("span no ray tubes"), "{warning}");
        Ok(())
    }
    #[test]
    fn report_contains_estimator_property() -> OpmResult<()> {
        use crate::{
            analyzers::RayTraceConfig, distributions::position::Hexapolar, joule, light::Rays,
            millimeter, nanometer,
        };

        let mut detector = FluenceDetector::default();
        detector.set_property("error estimates", true.into())?;
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
        let Proptype::F64(share) = props.get("Energy in edge margins")? else {
            panic!("Expected the share of the energy in the edge margins");
        };
        assert!(*share > 0.0 && *share < 1.0, "{share}");

        // Verify read_observable retrieves raw SI values
        let peak = detector.read_observable("peak fluence");
        assert!(peak.is_some());
        assert!(peak.unwrap() > 0.0);
        // the peak has no vector form, so reading one computes no second map
        assert_eq!(detector.read_observable_vector("peak fluence"), None);

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
    /// The fluence of the flat-top beam of [`map_of_a_flat_top`], in J/cm².
    fn flat_top_fluence() -> f64 {
        crate::utils::test_helper::helper::flat_top_fluence(BEAM_RADIUS, BEAM_RINGS)
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
            distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
            joule, millimeter,
            prelude::{CollimatedSrc, RayDataSource},
        };
        let source = RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(BEAM_RADIUS), BEAM_RINGS)?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(lines)?.into(),
        ));
        detector_map(source.into(), FluenceEstimator::Voronoi, bored_mirror)
    }
    /// Traces the given light into a fluence detector of the given estimator 150 mm from the
    /// source and returns the map the detector computes. With `bored_mirror`, a default bored
    /// mirror 100 mm from the source reflects the light back onto the detector 50 mm in front of
    /// it.
    fn detector_map(
        light: crate::light::lightdata::ray_data_builder::RayDataBuilder,
        estimator: FluenceEstimator,
        bored_mirror: bool,
    ) -> OpmResult<FluenceData> {
        use crate::core_optics::node_attr::HasNodeAttr;
        let mut detector = FluenceDetector::default();
        detector
            .node_attr_mut()
            .set_property("fluence estimator", estimator.into())?;
        traced(detector, light, bored_mirror)?
            .calc_fluence_data()?
            .ok_or_else(|| crate::error::OpossumError::Other("the detector has no map".into()))
    }
    /// Traces the given light into the given detector 150 mm from the source and returns the
    /// detector with what it recorded. With `bored_mirror`, a default bored mirror 100 mm from the
    /// source reflects the light back onto the detector 50 mm in front of it.
    fn traced(
        detector: FluenceDetector,
        light: crate::light::lightdata::ray_data_builder::RayDataBuilder,
        bored_mirror: bool,
    ) -> OpmResult<FluenceDetector> {
        use crate::{
            analyzers::{Analyzer, RayTraceConfig, raytrace::RayTracingAnalyzer},
            millimeter,
            nodes::{BoredMirror, NodeGroup, SourcePort},
        };
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let detector = scenery.add_node(detector)?;
        if bored_mirror {
            let mirror = scenery.add_node(BoredMirror::default())?;
            scenery.connect_nodes(source, "output_1", mirror, "mirror", millimeter!(100.0))?;
            scenery.connect_nodes(mirror, "in_front", detector, "input_1", millimeter!(50.0))?;
        } else {
            scenery.connect_nodes(source, "output_1", detector, "input_1", millimeter!(150.0))?;
        }
        let mut config = RayTraceConfig::default();
        config.map_source(source, light);
        RayTracingAnalyzer::new(config).analyze(&mut scenery)?;
        scenery
            .node(detector)?
            .as_any()
            .downcast_ref::<FluenceDetector>()
            .cloned()
            .ok_or_else(|| crate::error::OpossumError::Other("no fluence detector".into()))
    }
    /// A fluence detector that reports its error estimates.
    fn estimating_errors() -> OpmResult<FluenceDetector> {
        let mut detector = FluenceDetector::default();
        detector.set_property("error estimates", true.into())?;
        Ok(detector)
    }
    /// The entries of a report that tell how far the fluence may lie from the map.
    const ERROR_ESTIMATES: [&str; 4] = [
        "Fluence uncertainty",
        "Peak fluence uncertainty",
        "Energy deviation of the map",
        "Energy in edge margins",
    ];
    /// A detector reports its map, its peak, the energy that hit it and its estimator, but no
    /// error estimates unless asked.
    #[test]
    fn a_detector_reports_no_error_estimates_by_default() -> OpmResult<()> {
        let detector = traced(FluenceDetector::default(), grid_beam(false)?.into(), false)?;
        let report = report_of(&detector)?;
        for shown in ["Fluence", "Peak Fluence", "Total energy", "Estimator"] {
            assert!(report.contains(shown), "{shown}");
        }
        for hidden in ERROR_ESTIMATES {
            assert!(!report.contains(hidden), "{hidden}");
        }
        Ok(())
    }
    /// With error estimates switched on, a detector reports all of them.
    #[test]
    fn a_detector_reports_its_error_estimates_when_asked() -> OpmResult<()> {
        let report = report_of(&traced(
            estimating_errors()?,
            grid_beam(false)?.into(),
            false,
        )?)?;
        for shown in ERROR_ESTIMATES {
            assert!(report.contains(shown), "{shown}");
        }
        Ok(())
    }
    /// The properties of the ray trace report of the given detector.
    fn report_of(detector: &FluenceDetector) -> OpmResult<crate::properties::Properties> {
        let NodeReportResult::Report(report) =
            detector.node_report("test_uuid", AnalyzerKind::RayTrace)?
        else {
            return Err(crate::error::OpossumError::Other("no report".into()));
        };
        Ok(report.properties().clone())
    }
    /// A Gaussian beam of 1 J with a standard deviation of 2 mm, centered `center` mm along x and
    /// sampled by hexapolar rings out to 8 mm around the axis: 7 rings lie about 1.1 mm apart.
    fn gaussian_beam(rings: u8, center: f64) -> OpmResult<crate::prelude::RayDataSource> {
        use crate::{
            degree,
            distributions::{energy::General2DGaussian, position::Hexapolar, spectral::LaserLines},
            joule, millimeter,
            prelude::{CollimatedSrc, RayDataSource},
        };
        Ok(RayDataSource::Collimated(CollimatedSrc::new(
            Hexapolar::new(millimeter!(8.0), rings)?.into(),
            General2DGaussian::new(
                joule!(1.0),
                millimeter!(center, 0.0),
                millimeter!(2.0, 2.0),
                1.0,
                degree!(0.0),
                false,
            )?
            .into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        )))
    }
    /// The fluence of [`gaussian_beam`] at the given point in mm, in J/cm², from the shape the
    /// source samples, scaled to the given map where the middle ray of the beam arrived: each
    /// corner of the map shows the shape there times one factor (see the handbook,
    /// "Fluence uncertainty").
    fn gaussian_fluence(map: &FluenceData, center: f64) -> OpmResult<impl Fn(f64, f64) -> f64> {
        use crate::{degree, millimeter, utils::super_gaussian::SuperGaussianShape};
        let shape = SuperGaussianShape::new(
            millimeter!(center, 0.0),
            millimeter!(2.0, 2.0),
            1.0,
            degree!(0.0),
            false,
        )?;
        let scale = fluence_near(map, 0.0, 0.0)? / shape.value_at(&millimeter!(0.0, 0.0));
        Ok(move |x: f64, y: f64| scale * shape.value_at(&millimeter!(x, y)))
    }
    /// The largest distance, in J/cm², of the map of [`gaussian_beam`] and of the map plus its
    /// estimated deviation from the fluence of the beam, over the points within two standard
    /// deviations of its center.
    fn gaussian_errors(rings: u8, center: f64) -> OpmResult<(f64, f64)> {
        use uom::si::{length::millimeter, radiant_exposure::joule_per_square_centimeter};
        let map = traced(
            estimating_errors()?,
            gaussian_beam(rings, center)?.into(),
            false,
        )?
        .calc_fluence_data()?
        .ok_or_else(|| crate::error::OpossumError::Other("no map".into()))?;
        let fluence = gaussian_fluence(&map, center)?;
        let deviation = map
            .deviation()
            .ok_or_else(|| crate::error::OpossumError::Other("no deviation".into()))?;
        let (xs, ys, values) = map.get_fluence_distribution()?;
        let (mut linear, mut corrected) = (0.0_f64, 0.0_f64);
        for (row, y) in ys.iter().enumerate() {
            for (column, x) in xs.iter().enumerate() {
                let (x, y) = (x.get::<millimeter>(), y.get::<millimeter>());
                if (x - center).hypot(y) <= 4.0 {
                    let map = values[(row, column)].get::<joule_per_square_centimeter>();
                    let deviation = deviation[(row, column)].get::<joule_per_square_centimeter>();
                    linear = linear.max((map - fluence(x, y)).abs());
                    corrected = corrected.max((map + deviation - fluence(x, y)).abs());
                }
            }
        }
        Ok((linear, corrected))
    }
    /// Between the rays, the map plus its estimated deviation lies closer to a Gaussian beam than
    /// the map alone: about five times closer with rays 0.57 standard deviations apart.
    #[test]
    fn the_corrected_map_lies_closer_to_the_beam() -> OpmResult<()> {
        let (linear, corrected) = gaussian_errors(7, 0.0)?;
        assert!(
            3.0 * corrected < linear,
            "{corrected} J/cm², {linear} J/cm²"
        );
        Ok(())
    }
    /// Halving the spacing of the rays shrinks the deviation of the map from a Gaussian beam about
    /// fourfold, as the second order it is of, and what the estimate leaves about eightfold, as
    /// the third (see the handbook, "Fluence uncertainty").
    #[test]
    fn the_estimated_deviation_converges_one_order_faster() -> OpmResult<()> {
        let errors = [7, 14, 28]
            .into_iter()
            .map(|rings| gaussian_errors(rings, 0.0))
            .collect::<OpmResult<Vec<_>>>()?;
        for pair in errors.windows(2) {
            let [(linear, corrected), (finer_linear, finer_corrected)] = pair else {
                panic!("two spacings");
            };
            let (linear_ratio, corrected_ratio) =
                (linear / finer_linear, corrected / finer_corrected);
            assert!((2.5..4.5).contains(&linear_ratio), "{linear_ratio}");
            assert!(corrected_ratio > 6.0, "{corrected_ratio}");
        }
        Ok(())
    }
    /// The report tells how far the peak may lie above the peak of the map: for a Gaussian beam
    /// whose peak lies between the rays, the estimate closes most of the gap, though only as an
    /// estimate of the second order, not a bound.
    #[test]
    fn the_report_tells_the_uncertainty_of_the_peak() -> OpmResult<()> {
        use uom::si::radiant_exposure::joule_per_square_centimeter;
        let detector = traced(estimating_errors()?, gaussian_beam(7, 1.0)?.into(), false)?;
        let report = report_of(&detector)?;
        let fluence_in = |name: &str| -> OpmResult<f64> {
            let Proptype::Fluence(fluence) = report.get(name)?.clone() else {
                panic!("Expected a fluence for {name}");
            };
            Ok(fluence.get::<joule_per_square_centimeter>())
        };
        let (peak, uncertainty) = (
            fluence_in("Peak Fluence")?,
            fluence_in("Peak fluence uncertainty")?,
        );
        assert!(matches!(
            report.get("Fluence uncertainty")?,
            Proptype::FluenceData(_)
        ));
        let map = detector
            .calc_fluence_data()?
            .ok_or_else(|| crate::error::OpossumError::Other("no map".into()))?;
        let truth = gaussian_fluence(&map, 1.0)?(1.0, 0.0);
        let gap = truth - peak;
        assert!(
            gap > 0.0,
            "the peak of the map lies between the rays, below the beam's"
        );
        assert!(
            (gap - uncertainty).abs() < 0.3 * gap,
            "{uncertainty} J/cm² of {gap} J/cm²"
        );
        Ok(())
    }
    /// A flat-top beam deviates nowhere from its map, so its peak is certain.
    #[test]
    fn a_flat_top_reports_no_uncertainty() -> OpmResult<()> {
        let detector = traced(estimating_errors()?, grid_beam(false)?.into(), false)?;
        let Proptype::Fluence(uncertainty) = report_of(&detector)?
            .get("Peak fluence uncertainty")?
            .clone()
        else {
            panic!("Expected the peak fluence uncertainty");
        };
        assert!(uncertainty.value.abs() < 1e-9, "{uncertainty:?}");
        Ok(())
    }
    /// KDE and binning maps follow no ray tubes and tell no uncertainty.
    #[test]
    fn kde_and_binning_tell_no_uncertainty() -> OpmResult<()> {
        use crate::core_optics::node_attr::HasNodeAttr;
        for estimator in [FluenceEstimator::KDE, FluenceEstimator::Binning] {
            let mut detector = FluenceDetector::default();
            detector
                .node_attr_mut()
                .set_property("fluence estimator", estimator.into())?;
            let report = report_of(&traced(detector, grid_beam(false)?.into(), false)?)?;
            assert!(!report.contains("Fluence uncertainty"));
            assert!(!report.contains("Peak fluence uncertainty"));
        }
        Ok(())
    }
    /// The report and the metric of a detector tell the energy that hit it, which is the energy of
    /// the source: neither the points of the map nor the interpolation between its rays bend it.
    #[test]
    fn the_report_tells_the_energy_that_hit_the_detector() -> OpmResult<()> {
        for source in [grid_beam(false)?, gaussian_beam(7, 0.0)?] {
            let detector = traced(FluenceDetector::default(), source.into(), false)?;
            let Proptype::Energy(energy) = report_of(&detector)?.get("Total energy")?.clone()
            else {
                panic!("Expected the total energy");
            };
            approx::assert_relative_eq!(energy.value, 1.0, max_relative = 1e-9);
            let metric = detector.read_observable("total energy").unwrap_or(f64::NAN);
            approx::assert_relative_eq!(metric, 1.0, max_relative = 1e-9);
        }
        Ok(())
    }
    /// A detector with a window smaller than the beam tells the energy of the rays within it: 19 ×
    /// 19 of the 31 × 31 rays of the grid, each standing for a square of 0.533 mm, of a beam
    /// reaching half a ray spacing beyond its 16 mm.
    #[test]
    fn a_detector_tells_the_energy_within_its_window() -> OpmResult<()> {
        use crate::{apertures::RectangleShape, millimeter};
        let mut detector = FluenceDetector::default();
        let window: crate::apertures::OpticShape =
            RectangleShape::new(millimeter!(10.0), millimeter!(10.0))?.into();
        detector.set_property(OPTIC_SHAPE, window.into())?;
        let detector = traced(detector, grid_beam(false)?.into(), false)?;
        let Proptype::Energy(energy) = report_of(&detector)?.get("Total energy")?.clone() else {
            panic!("Expected the total energy");
        };
        let spacing: f64 = 16.0 / 30.0;
        let within = (19.0 * spacing / (16.0 + spacing)).powi(2);
        approx::assert_relative_eq!(energy.value, within, max_relative = 1e-9);
        Ok(())
    }
    /// Without a given range, a map is square, centered on its light and 5 % wider than the light
    /// along its longer side, so its points do not lie on the edges of the light and a square
    /// beam is drawn alike on all four sides.
    #[test]
    fn a_map_is_square_and_reaches_a_little_beyond_its_light() -> OpmResult<()> {
        let map = detector_map(grid_beam(false)?.into(), FluenceEstimator::Voronoi, false)?;
        // the light reaches half a ray spacing beyond the 16 mm of the grid
        let side = 1.05 * (16.0 + 16.0 / 30.0) * 1e-3;
        for range in [map.x_range(), map.y_range()] {
            approx::assert_relative_eq!((range.end - range.start).value, side, max_relative = 1e-9);
            approx::assert_abs_diff_eq!((range.start + range.end).value, 0.0, epsilon = 1e-12);
        }
        let values = map.interp_distribution().map(|fluence| fluence.value);
        let (last_row, last_column) = (values.nrows() - 1, values.ncols() - 1);
        let tolerance = 1e-9 * values.max();
        for row in 0..=last_row {
            for column in 0..=last_column {
                let value = values[(row, column)];
                approx::assert_abs_diff_eq!(
                    value,
                    values[(row, last_column - column)],
                    epsilon = tolerance
                );
                approx::assert_abs_diff_eq!(
                    value,
                    values[(last_row - row, column)],
                    epsilon = tolerance
                );
            }
        }
        Ok(())
    }
    /// The report tells how far the energy the map holds misses the energy that hit: not at all for
    /// a flat-top beam, and for a coarsely sampled Gaussian as much as the map itself holds more,
    /// which a finely rastered map of it confirms.
    #[test]
    fn the_report_tells_how_far_the_map_misses_that_energy() -> OpmResult<()> {
        let deviation_of = |detector: &FluenceDetector| -> OpmResult<f64> {
            let Proptype::F64(deviation) = report_of(detector)?
                .get("Energy deviation of the map")?
                .clone()
            else {
                panic!("Expected the energy deviation of the map");
            };
            Ok(deviation)
        };
        let flat = traced(estimating_errors()?, grid_beam(false)?.into(), false)?;
        assert!(deviation_of(&flat)?.abs() < 1e-9);
        let gaussian = traced(estimating_errors()?, gaussian_beam(7, 0.0)?.into(), false)?;
        let fine = gaussian
            .get_optic_surface("input_1")
            .ok_or_else(|| crate::error::OpossumError::Other("no surface".into()))?
            .hit_map()
            .calc_fluence_map((1001, 1001), &FluenceEstimator::Voronoi)?
            .total_energy()
            .value;
        approx::assert_abs_diff_eq!(deviation_of(&gaussian)?, fine - 1.0, epsilon = 1e-4);
        Ok(())
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
    /// A flat-top beam is even all over: in its middle, where the middle ray of the hexapolar
    /// pattern stands for less area than the rays of its rings and so carries less energy, and
    /// between its rings, whose rays stand for unequal areas.
    #[test]
    fn a_flat_top_beam_is_even_all_over() -> OpmResult<()> {
        use uom::si::{length::millimeter, radiant_exposure::joule_per_square_centimeter};
        let map = map_of_a_flat_top(vec![(nanometer!(1000.0), 1.0)], false)?;
        let middle = fluence_near(&map, 0.0, 0.0)?;
        approx::assert_relative_eq!(middle, flat_top_fluence(), max_relative = 0.02);
        let (xs, ys, values) = map.get_fluence_distribution()?;
        for (row, y) in ys.iter().enumerate() {
            for (column, x) in xs.iter().enumerate() {
                let (x, y) = (x.get::<millimeter>(), y.get::<millimeter>());
                if x.hypot(y) <= BEAM_RADIUS {
                    let value = values[(row, column)].get::<joule_per_square_centimeter>();
                    assert!(
                        (value / middle - 1.0).abs() < 1e-9,
                        "{value} J/cm² at ({x:.2}, {y:.2}) mm, {middle} J/cm² in the middle"
                    );
                }
            }
        }
        Ok(())
    }
    /// A flat-top beam of 1 J on a grid of 31 × 31 rays over 16 mm, which fits the window of a
    /// detector also turned, sent out collimated or, with `point`, from a point whose rays reach the
    /// grid 150 mm away.
    fn grid_beam(point: bool) -> OpmResult<crate::prelude::RayDataSource> {
        use crate::{
            distributions::{energy::UniformDist, position::Grid, spectral::LaserLines},
            joule, millimeter,
            prelude::{CollimatedSrc, PointSrc, RayDataSource},
        };
        let (grid, energy, lines) = (
            Grid::new(millimeter!(16.0, 16.0), nalgebra::Point2::new(31, 31))?.into(),
            UniformDist::new(joule!(1.0))?.into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        );
        Ok(if point {
            RayDataSource::PointSrc(PointSrc::new(grid, energy, lines, millimeter!(150.0))?)
        } else {
            RayDataSource::Collimated(CollimatedSrc::new(grid, energy, lines))
        })
    }
    /// A grid shows the same fluence from a point source as from a collimated one, its edges with
    /// the same margins: across the directions of a point source, the rays along an edge lie on
    /// one line only up to rounding, and the flat triangles spade lays between them are no tubes.
    #[test]
    fn a_grid_from_a_point_source_shows_the_fluence_of_a_collimated_one() -> OpmResult<()> {
        let collimated = detector_map(grid_beam(false)?.into(), FluenceEstimator::Voronoi, false)?;
        let point = detector_map(grid_beam(true)?.into(), FluenceEstimator::Voronoi, false)?;
        approx::assert_relative_eq!(
            fluence_near(&point, 0.0, 0.0)?,
            fluence_near(&collimated, 0.0, 0.0)?,
            max_relative = 1e-6
        );
        let share = |map: &FluenceData| map.edge_share().expect("a share of the margins");
        approx::assert_relative_eq!(share(&point), share(&collimated), max_relative = 1e-6);
        Ok(())
    }
    /// A uniform point source shows a flat map whatever pattern its rays follow: its tubes are
    /// spanned across the axis of the source, where its distribution lies, also for patterns whose
    /// mean direction leans off that axis.
    #[test]
    fn a_uniform_point_source_shows_a_flat_map_for_any_pattern() -> OpmResult<()> {
        use crate::{
            distributions::{
                energy::UniformDist,
                position::{FibonacciEllipse, PosDistType, Random},
                spectral::LaserLines,
            },
            joule, millimeter,
            prelude::{PointSrc, RayDataSource},
        };
        let patterns: [PosDistType; 2] = [
            Random::new(millimeter!(8.0), millimeter!(8.0), 800)?.into(),
            FibonacciEllipse::new(millimeter!(8.0), millimeter!(8.0), 800)?.into(),
        ];
        for pattern in patterns {
            let source = RayDataSource::PointSrc(PointSrc::new(
                pattern,
                UniformDist::new(joule!(1.0))?.into(),
                LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
                millimeter!(150.0),
            )?);
            let map = detector_map(source.into(), FluenceEstimator::Voronoi, false)?;
            let middle = fluence_near(&map, 0.0, 0.0)?;
            for (x, y) in [(3.0, 0.0), (-4.0, 2.0), (1.0, -5.0), (-2.5, -3.5)] {
                approx::assert_relative_eq!(fluence_near(&map, x, y)?, middle, max_relative = 1e-9);
            }
        }
        Ok(())
    }
    /// A grid its builder turns about the axis shows the map of an upright one, its edges with the
    /// same margins and without folding over itself, though its rays along an edge arrive on one
    /// line only up to rounding.
    #[test]
    fn a_turned_grid_shows_the_map_of_an_upright_one() -> OpmResult<()> {
        use crate::{degree, utils::geom_transformation::Isometry};
        let upright = detector_map(grid_beam(false)?.into(), FluenceEstimator::Voronoi, false)?;
        let mut light =
            crate::light::lightdata::ray_data_builder::RayDataBuilder::from(grid_beam(false)?);
        light.set_isometry(Some(Isometry::new_rotation(degree!(0.0, 0.0, 30.0))?));
        testing_logger::setup();
        let turned = detector_map(light, FluenceEstimator::Voronoi, false)?;
        testing_logger::validate(|logs| {
            let folds = logs
                .iter()
                .filter(|log| log.body.contains("folds over itself"));
            assert_eq!(
                folds.count(),
                0,
                "{:?}",
                logs.iter().map(|l| &l.body).collect::<Vec<_>>()
            );
        });
        approx::assert_relative_eq!(
            fluence_near(&turned, 0.0, 0.0)?,
            fluence_near(&upright, 0.0, 0.0)?,
            max_relative = 1e-6
        );
        let share = |map: &FluenceData| map.edge_share().expect("a share of the margins");
        approx::assert_relative_eq!(share(&turned), share(&upright), max_relative = 1e-6);
        Ok(())
    }
    /// A Gaussian beam on a Fibonacci pattern shows its exact peak: each ray carries the beam
    /// over the area it stands for, which the innermost rays of the pattern stand for less of.
    #[test]
    fn a_gaussian_beam_on_a_fibonacci_pattern_shows_its_peak() -> OpmResult<()> {
        use crate::{
            degree,
            distributions::{
                energy::General2DGaussian, position::FibonacciEllipse, spectral::LaserLines,
            },
            joule, millimeter,
            prelude::{CollimatedSrc, RayDataSource},
        };
        use uom::si::radiant_exposure::joule_per_square_centimeter;
        let source = RayDataSource::Collimated(CollimatedSrc::new(
            FibonacciEllipse::new(millimeter!(8.0), millimeter!(8.0), 2000)?.into(),
            General2DGaussian::new(
                joule!(1.0),
                millimeter!(0.0, 0.0),
                millimeter!(2.0, 2.0),
                1.0,
                degree!(0.0),
                false,
            )?
            .into(),
            LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
        ));
        let map = detector_map(source.into(), FluenceEstimator::Voronoi, false)?;
        // 1 J over 2 π σ², with σ = 0.2 cm
        let peak = 1.0 / (2.0 * std::f64::consts::PI * 0.04);
        approx::assert_relative_eq!(
            map.peak().get::<joule_per_square_centimeter>(),
            peak,
            max_relative = 0.02
        );
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
