#![warn(missing_docs)]
//! Wavefront measurement node
pub mod wavefront_data;
pub mod zernike;

use crate::{
    analyzers::{
        AnalyzerKind,
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{
        NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, Planar, PortType, SurfacePorts,
        SurfaceKind,
        optic_node::APERTURE_LIMIT_NOTE,
    },
    error::{OpmResult, OpossumError},
    geometry::{
        Geometry, SurfaceShape,
        body::{OPTIC_SHAPE, default_square_optic_shape},
    },
    light::{LightData, LightResult},
    millimeter,
    nodes::{
        NodeRegistration,
        wavefront::zernike::{WavefrontDecomposition, ZernikeNoll},
    },
    properties::{Properties, Proptype},
    reporting::node_report::{NodeReport, NodeReportResult},
    utils::geom_transformation::Isometry,
};
use log::warn;
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use wavefront_data::WaveFrontMap;

inventory::submit! {
    NodeRegistration::new::<WaveFront>("wavefront monitor", "wavefront detector")
}

/// A wavefront monitor node
///
/// This node creates a wavefront view of an incoming ray bundle and can be used as an ideal wavefront-measurement device
///
/// ## Optical Ports
///   - Inputs
///     - `in1`
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `optic shape`: the window the light is recorded within, a 25 mm square by default; light
///     outside it passes unrecorded
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(OpmNode, Serialize, Deserialize, Clone, Debug)]
#[opm_node("goldenrod1")]
pub struct WaveFront {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
    /// Optional custom reference surface for wavefront evaluation (e.g. a sphere), with its vertex
    /// at the node.
    #[serde(skip)]
    reference_surface: Option<SurfaceShape>,
}

impl Default for WaveFront {
    /// create a wavefront monitor.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("wavefront monitor");
        node_attr
            .create_property(
                "remove tilt",
                "remove a wavefront tilt (plane surface)",
                false.into(),
            )
            .unwrap();
        node_attr
            .create_property(
                OPTIC_SHAPE,
                "window the light is recorded within",
                default_square_optic_shape().into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "decompose zernike",
                "decompose wavefront into Zernike polynomials",
                false.into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "zernike max order",
                "maximum radial order n for Zernike decomposition",
                4.into(), // Default n=4 yields 15 modes up to spherical aberration
            )
            .unwrap();
        let mut wf = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
            reference_surface: None,
        };
        wf.create_default_surface_properties().unwrap();
        wf.update_surfaces().unwrap();
        wf
    }
}
impl WaveFront {
    /// Creates a new [`WaveFront`] Monitor with the given `name`.
    /// # Attributes
    /// - `name`: name of the [`WaveFront`] Monitor
    /// # Errors
    /// This function returns an error if `update_surfaces` fails.
    pub fn new(name: &str) -> OpmResult<Self> {
        let mut wf = Self::default();
        wf.node_attr.set_name(name);
        wf.update_surfaces()?;
        Ok(wf)
    }
    /// Sets a custom reference surface for wavefront calculation.
    ///
    /// Allows analyzing wavefronts against curved surfaces like spheres. The surface's vertex sits
    /// at the node, so it moves with the node.
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the reference surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface cannot be built from the given shape.
    pub fn set_reference_surface(&mut self, shape: SurfaceShape) -> OpmResult<()> {
        self.reference_surface = Some(shape);
        self.update_surfaces()
    }
    /// Helper function to evaluate and append polynomial basis decomposition to the report properties.
    fn add_decomposition_properties(
        props: &mut Properties,
        wf_error_map: &WaveFrontMap,
        order: usize,
    ) -> OpmResult<()> {
        let norm_radius = wf_error_map.max_radius();
        if norm_radius <= millimeter!(1.0e-9) {
            props.create(
                "Warning",
                "warning during decomposition",
                "Beam radius is near zero; cannot fit basis functions.".into(),
            )?;
            return Ok(());
        }

        let basis = ZernikeNoll::new(order, norm_radius);
        match wf_error_map.decompose(&basis) {
            Ok(decomp) => {
                let wvl_nm = wf_error_map
                    .wavelength()
                    .get::<uom::si::length::nanometer>();
                let decomp_prop = WavefrontDecomposition::new(decomp, wf_error_map.wavelength());

                props.create(
                    &format!(
                        "{} Decomposition at {wvl_nm:.3} nm",
                        decomp_prop.decomposition.basis_name
                    ),
                    "Wavefront decomposition into orthogonal polynomial basis modes",
                    decomp_prop.into(),
                )?;
            }
            Err(err) => {
                props.create(
                    "Warning",
                    "warning during wavefront decomposition",
                    format!("Failed to calculate basis coefficients: {err}").into(),
                )?;
            }
        }
        Ok(())
    }
}

impl Planar for WaveFront {
    fn surface_kind(&self) -> SurfaceKind {
        SurfaceKind::Detector
    }
}
impl OpticNode for WaveFront {
    /// The surface wavefronts are measured on: the reference surface, or a plane if none is set,
    /// within the recording window.
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::detector(
            self.reference_surface
                .clone()
                .unwrap_or(SurfaceShape::Plane),
            self.cross_section()?,
        )?))
    }
    fn as_surface(&self) -> Option<&dyn Planar> {
        Some(self)
    }
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::SINGLE_SURFACE
    }
    fn node_report(&self, uuid: &str, analyzer: AnalyzerKind) -> OpmResult<NodeReportResult> {
        if analyzer == AnalyzerKind::Energy {
            return Ok(NodeReportResult::incompatible_warning(format!(
                "Node '{}' ({}): A wavefront plot can only be calculated during a ray tracing or ghostfocus analysis.",
                self.name(),
                self.node_type()
            )));
        }
        let mut props = Properties::default();
        let data = &self.light_data;

        if let Some(LightData::Geometric(rays)) = data {
            let iso = self
                .effective_surface_iso("input_1")
                .unwrap_or_else(|_| Isometry::identity());

            let Ok(Proptype::Bool(remove_tilt)) = self.node_attr.get_property("remove tilt") else {
                return Err(OpossumError::Analysis(
                    "cannot read `remove tilt` flag".into(),
                ));
            };
            let Ok(Proptype::Bool(decomp_zernike)) =
                self.node_attr.get_property("decompose zernike")
            else {
                return Err(OpossumError::Analysis(
                    "cannot read `decompose zernike` flag".into(),
                ));
            };
            let Ok(Proptype::I32(max_order)) = self.node_attr.get_property("zernike max order")
            else {
                return Err(OpossumError::Analysis(
                    "cannot read `zernike max order`".into(),
                ));
            };
            let zernike_order = usize::try_from(*max_order).unwrap_or(0);

            let wf_data_opt =
                wavefront_data::WaveFrontData::from_rays(rays, true, false, &iso, *remove_tilt);

            if let Ok(ref wf_data) = wf_data_opt
                && !wf_data.wavefront_error_maps.is_empty()
            {
                for wf_error_map in &wf_data.wavefront_error_maps {
                    let wvl_nm = wf_error_map
                        .wavelength()
                        .get::<uom::si::length::nanometer>();

                    props.create(
                        &format!("Wavefront Map at {wvl_nm:.3} nm"),
                        "Wavefront error map with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        wf_error_map.clone().into(),
                    )?;
                    props.create(
                        "Wavefront PtV",
                        "Wavefront Peak-to-Valley value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.ptv(), wf_error_map.wavelength()),
                    )?;
                    props.create(
                        "Wavefront RMS",
                        "Wavefront root mean square value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.rms(), wf_error_map.wavelength()),
                    )?;

                    if *remove_tilt {
                        props.create(
                            "Note",
                            "note",
                            "A possible wavefront tilt has been subtracted.".into(),
                        )?;
                    }

                    if *decomp_zernike {
                        Self::add_decomposition_properties(
                            &mut props,
                            wf_error_map,
                            zernike_order,
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
                props.create(
                    "Warning",
                    "warning during wavefront calculation",
                    "This warning might have been created if the wavefront monitor was used with zero distance from Source or with multiple wavelengths in a completely paraxial setup.".into(),
                )?;
            }
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
}
impl From<WaveFrontMap> for Proptype {
    fn from(value: WaveFrontMap) -> Self {
        Self::WaveFrontData(value)
    }
}
impl AnalysisGhostFocus for WaveFront {}
impl AnalysisEnergy for WaveFront {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let result = self.unified_analyze_single_surface_node(incoming_data, config, "input_1")?;
        let out_port = &self.ports().names(&PortType::Output)[0];
        if let Some(data) = result.get(out_port) {
            self.light_data = Some(data.clone());
        }
        Ok(result)
    }
}
impl AnalysisRayTrace for WaveFront {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig,
        core_optics::{PortType, node_attr::NodePositioning},
        distributions::position::Hexapolar,
        joule,
        light::{Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::test_helper::helper::*,
        utils::geom_transformation::Isometry,
    };
    #[test]
    fn default() {
        let node = WaveFront::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "wavefront monitor");
        assert_eq!(node.node_type(), "wavefront monitor");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "goldenrod1");
    }
    #[test]
    fn new() -> OpmResult<()> {
        let meter = WaveFront::new("test")?;
        assert_eq!(meter.name(), "test");
        assert!(meter.light_data.is_none());
        Ok(())
    }
    #[test]
    fn ports() {
        let meter = WaveFront::default();
        assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut meter = WaveFront::default();
        meter.set_inverted(true)?;
        assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<WaveFront>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<WaveFront>()
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = WaveFront::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_ok() -> OpmResult<()> {
        let mut node = WaveFront::default();
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.), 1)?,
        )?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input.clone(), &EnergyConfig::default());
        assert!(output.is_ok());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default());
        assert!(output.is_ok());
        let output = output?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        Ok(())
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<WaveFront>()
    }
    #[test]
    fn records_inside_and_passes_all() -> OpmResult<()> {
        test_detector_records_inside_and_passes_all::<WaveFront>(|monitor| {
            monitor.light_data.as_ref()
        })
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = WaveFront::default();
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output_map = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert_eq!(output_map.len(), 1);
        let result_light = output_map.get("input_1");
        assert_eq!(result_light, Some(&input_light));
        Ok(())
    }
    #[test]
    fn report() -> OpmResult<()> {
        let mut wf = WaveFront::default();
        assert!(matches!(
            wf.node_report("", AnalyzerKind::Energy)?,
            NodeReportResult::Incompatible(_)
        ));
        wf.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.), 1)?,
        )?));
        let NodeReportResult::Report(node_report) = wf.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Wrong report variant");
        };
        assert_eq!(node_report.node_type(), "wavefront monitor");
        assert_eq!(node_report.name(), "wavefront monitor");
        let props = node_report.properties();
        assert!(props.contains("Wavefront Map at 1053.000 nm"));
        assert!(props.contains("Wavefront RMS"));
        assert!(props.contains("Wavefront PtV"));
        let nr_of_props = props.iter().count();
        assert_eq!(nr_of_props, 3);

        Ok(())
    }
    #[test]
    fn report_with_zernike() -> OpmResult<()> {
        let mut wf = WaveFront::default();
        wf.node_attr
            .set_property("decompose zernike", true.into())?;
        wf.node_attr.set_property("zernike max order", 2.into())?; // Order 2: 6 modes (j=1..6)

        // Generate hexapolar rays with 2 rings (enough points for 6 modes)
        wf.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(2.0), 2)?,
        )?));

        let NodeReportResult::Report(node_report) = wf.node_report("", AnalyzerKind::RayTrace)?
        else {
            panic!("Wrong report variant");
        };

        let props = node_report.properties();
        assert!(props.contains("Wavefront Map at 1053.000 nm"));
        assert!(props.contains("Wavefront RMS"));
        assert!(props.contains("Wavefront PtV"));
        assert!(props.contains("Zernike Decomposition at 1053.000 nm"));
        assert_eq!(props.nr_of_props(), 4);
        Ok(())
    }
}
