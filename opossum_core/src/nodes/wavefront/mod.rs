#![warn(missing_docs)]
//! Wavefront measurement node
pub mod wavefront_data;

use crate::{
    analyzers::{
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, OpticNode, PortType},
    error::{OpmResult, OpossumError},
    geometry::geo_surface::GeoSurfaceRef,
    light::{LightData, LightResult},
    nodes::NodeRegistration,
    properties::{Properties, Proptype},
    reporting::node_report::NodeReport,
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
///
/// During analysis, the output port contains a replica of the input port similar to a [`Dummy`](crate::nodes::Dummy) node. This way,
/// different dectector nodes can be "stacked" or used somewhere within the optical setup.
#[derive(OpmNode, Serialize, Deserialize, Clone, Debug)]
#[opm_node("goldenrod1")]
pub struct WaveFront {
    light_data: Option<LightData>,
    node_attr: NodeAttr,
    apodization_warning: bool,
    /// Optional custom reference surface for wavefront evaluation (e.g. Sphere).
    #[serde(skip)]
    reference_surface: Option<GeoSurfaceRef>,
}
unsafe impl Send for WaveFront {}

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
        let mut wf = Self {
            light_data: None,
            node_attr,
            apodization_warning: false,
            reference_surface: None, // Use standard plane surface as reference
        };
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
    /// Sets a custom geometric reference surface for wavefront calculation.
    /// Allows analyzing wavefronts against curved surfaces like spheres.
    ///
    /// # Errors
    ///
    /// Ths funtion returns an error if the internal update with the given surface reference fails.
    pub fn set_reference_surface(&mut self, geo: GeoSurfaceRef) -> OpmResult<()> {
        self.reference_surface = Some(geo);
        self.update_surfaces()
    }
}

impl OpticNode for WaveFront {
    fn set_apodization_warning(&mut self, apodized: bool) {
        self.apodization_warning = apodized;
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        // Use custom reference surface if defined, otherwise default to a Plane
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let geosurface = self.reference_surface.clone().unwrap_or_else(|| {
            GeoSurfaceRef(std::sync::Arc::new(std::sync::Mutex::new(
                crate::geometry::Plane::new(node_iso),
            )))
        });

        self.update_surface(
            &"input_1".to_string(),
            geosurface.clone(),
            Isometry::identity(),
            &PortType::Input,
        )?;
        self.update_surface(
            &"output_1".to_string(),
            geosurface,
            Isometry::identity(),
            &PortType::Output,
        )?;
        Ok(())
    }
    fn node_report(&self, uuid: &str) -> OpmResult<Option<NodeReport>> {
        let mut props = Properties::default();
        let data = &self.light_data;
        if let Some(LightData::Geometric(rays)) = data {
            let iso = self
                .effective_surface_iso("input_1")
                .unwrap_or_else(|_| Isometry::identity());
            let Ok(Proptype::Bool(remove_tilt)) = self.node_attr.get_property("remove tilt") else {
                return Err(OpossumError::Analysis(
                    "cannot read `remove tilt`collimation flag".into(),
                ));
            };
            let wf_data_opt =
                wavefront_data::WaveFrontData::from_rays(rays, true, false, &iso, *remove_tilt);

            if let Ok(ref wf_data) = wf_data_opt
                && !wf_data.wavefront_error_maps.is_empty()
            {
                for wf_error_map in &wf_data.wavefront_error_maps {
                    props
                    .create(
                        &format!("Wavefront Map at {:.3} nm", wf_error_map.wavelength().get::<uom::si::length::nanometer>()),
                        "Wavefront error map with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        wf_error_map.clone().into(),
                    )
                    ?;

                    //todo for all error maps at every wavelength!
                    props
                    .create(
                        &format!("Wavefront PtV at {:.3} nm", wf_error_map.wavelength().get::<uom::si::length::nanometer>()),
                        "Wavefront Peak-to-Valley value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.ptv(), wf_error_map.wavelength()),
                    )
                    ?;

                    //todo for all error maps at every wavelength!
                    props
                    .create(
                        &format!("Wavefront RMS at {:.3} nm", wf_error_map.wavelength().get::<uom::si::length::nanometer>()),
                        "Wavefront root mean square value with respect to the chief ray (closest ray to the optical axis) for a specific spectral band",
                        Proptype::WfLambda(wf_error_map.rms(), wf_error_map.wavelength()),
                    )
                    ?;
                    if *remove_tilt {
                        props.create(
                            "Note",
                            "note",
                            "A possible wavefront tilt has been subtracted.".into(),
                        )?;
                    }
                }

                if self.apodization_warning {
                    props.create(
                        "Warning",
                        "warning during analysis",
                        "Rays have been apodized at input aperture. Results might not be accurate."
                            .into(),
                    )?;
                }
            } else {
                props
                .create(
                    "Warning",
                    "warning during wavefront calculation",
                    "This warning might have been created if the Wavefront monitor was used with zero distance from Source or with multiple wavelengths in a completely paraxial setup.".into(),
                )
                ?;
            }

            Ok(Some(NodeReport::new(
                &self.node_type(),
                &self.name(),
                uuid,
                props,
            )))
        } else {
            Ok(None)
        }
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn reset_data(&mut self) {
        self.light_data = None;
        self.reset_optic_surfaces();
    }
    fn set_light_data(&mut self, ld: LightData) {
        self.light_data = Some(ld);
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
        let result =
            self.unified_analyze_single_surface_node(incoming_data, config, "input_1", None)?;
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
        core_optics::PortType,
        distributions::position::Hexapolar,
        joule,
        light::{Rays, spectrum_helper::create_he_ne_spec},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        utils::geom_transformation::Isometry,
    };
    #[test]
    fn default() {
        let mut node = WaveFront::default();
        assert!(node.light_data.is_none());
        assert_eq!(node.name(), "wavefront monitor");
        assert_eq!(node.node_type(), "wavefront monitor");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "goldenrod1");
        assert!(node.as_group_mut().is_err());
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
        node.set_isometry(Isometry::identity())?;
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
    fn analyze_apodazation_warning() -> OpmResult<()> {
        test_analyze_apodization_warning::<WaveFront>()
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
        assert!(wf.node_report("")?.is_none());
        wf.light_data = Some(LightData::Geometric(Rays::default()));
        assert!(wf.node_report("")?.is_some());
        wf.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1053.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.), 1)?,
        )?));
        let node_report = wf.node_report("")?.unwrap();
        assert_eq!(node_report.node_type(), "wavefront monitor");
        assert_eq!(node_report.name(), "wavefront monitor");
        let props = node_report.properties();
        assert!(props.contains("Wavefront Map at 1053.000 nm"));
        assert!(props.contains("Wavefront RMS at 1053.000 nm"));
        assert!(props.contains("Wavefront PtV at 1053.000 nm"));
        let nr_of_props = props.iter().count();
        assert_eq!(nr_of_props, 3);

        Ok(())
    }
}
