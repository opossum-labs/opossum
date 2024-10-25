#![warn(missing_docs)]
use log::{info, warn};
use uom::si::f64::Length;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, GhostFocusConfig, RayTraceConfig,
    },
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    joule,
    light_result::{LightRays, LightResult},
    lightdata::LightData,
    millimeter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::{OpticPorts, PortType},
    properties::Proptype,
    ray::Ray,
    rays::Rays,
    surface::{hit_map::HitMap, OpticalSurface, Plane},
    utils::{geom_transformation::Isometry, EnumProxy},
};
use std::{collections::HashMap, fmt::Debug};

/// A general light source
///
/// Hence it has only one output port (out1) and effectively no input ports. The formal input port `in1` is discarded during analysis.
/// Source nodes usually are the first nodes of a [`NodeGroup`](crate::nodes::NodeGroup).
///
/// ## Optical Ports
///   - Inputs
///     - `in1` (input discarded, used to make the node invertable)
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `light data`
///
/// **Note**: If a [`Source`] is configured as `inverted` the initial output port becomes an input port and further data is discarded.
#[derive(Clone)]
pub struct Source {
    node_attr: NodeAttr,
    surface: OpticalSurface,
}
impl Default for Source {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("source");
        node_attr
            .create_property(
                "light data",
                "data of the emitted light",
                None,
                EnumProxy::<Option<LightData>> { value: None }.into(),
            )
            .unwrap();

        node_attr
            .create_property(
                "alignment wavelength",
                "wavelength to be used for alignment. Necessary for, e.g., grating alignments",
                None,
                Proptype::LengthOption(None),
            )
            .unwrap();

        let mut ports = OpticPorts::new();
        ports.add(&PortType::Output, "out1").unwrap();
        ports.add(&PortType::Input, "in1").unwrap();
        node_attr.set_ports(ports);
        Self {
            node_attr,
            surface: OpticalSurface::new(Box::new(Plane::new(&Isometry::identity()))),
        }
    }
}
impl Source {
    /// Creates a new [`Source`].
    ///
    /// The light to be emitted from this source is defined in a [`LightData`] structure.
    ///
    /// # Panics
    /// Panics if [`Properties`](crate::properties::Properties) `name` can not be set
    ///
    /// ## Example
    ///
    /// ```rust
    /// use opossum::{
    /// lightdata::{DataEnergy, LightData},
    /// nodes::Source,
    /// spectrum_helper::create_he_ne_spec};
    ///
    /// let source=Source::new("My Source", &LightData::Energy(DataEnergy {spectrum: create_he_ne_spec(1.0).unwrap()}));
    /// ```
    #[must_use]
    pub fn new(name: &str, light: &LightData) -> Self {
        let mut source = Self::default();
        source.node_attr.set_name(name);
        source
            .node_attr
            .set_property(
                "light data",
                EnumProxy::<Option<LightData>> {
                    value: Some(light.clone()),
                }
                .into(),
            )
            .unwrap();
        source
    }

    /// Sets the alignment wavelength for an optical scenery
    /// This function is useful, or example, when aligning grating setups that should be analyzed with a given spectrum,
    /// but should be positioned to be ideal for a certain wavelength
    /// # Errors
    /// This function only propagates the errors of the contained functions
    pub fn set_alignment_wavelength(&mut self, wvl: Length) -> OpmResult<()> {
        self.node_attr
            .set_property("alignment wavelength", Proptype::LengthOption(Some(wvl)))
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    ///
    /// # Attributes
    /// * `light_data`: [`LightData`] that shall be set
    ///
    /// # Errors
    /// This function returns an error if the property "light data" can not be set
    pub fn set_light_data(&mut self, light_data: &LightData) -> OpmResult<()> {
        self.node_attr.set_property(
            "light data",
            EnumProxy::<Option<LightData>> {
                value: Some(light_data.clone()),
            }
            .into(),
        )?;
        Ok(())
    }
}

impl Alignable for Source {}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.node_attr.get_property("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop {
            &data.value
        } else {
            &None
        };
        match data {
            Some(data) => write!(f, "Source: {data}"),
            None => write!(f, "Source: no data"),
        }
    }
}
impl OpticNode for Source {
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn hit_maps(&self) -> HashMap<String, HitMap> {
        let mut maps = HashMap::default();
        maps.insert("out1".to_string(), self.surface.hit_map().to_owned());
        maps
    }
    fn get_surface_mut(&mut self, _surf_name: &str) -> &mut OpticalSurface {
        &mut self.surface
    }
}

impl Dottable for Source {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
impl LIDT for Source {}
impl Analyzable for Source {}
impl AnalysisEnergy for Source {
    fn analyze(&mut self, _incoming_data: LightResult) -> OpmResult<LightResult> {
        if let Ok(Proptype::LightData(data)) = self.node_attr.get_property("light data") {
            let Some(data) = data.value.clone() else {
                return Err(OpossumError::Analysis(
                    "source has empty light data defined".into(),
                ));
            };
            Ok(LightResult::from([("out1".into(), data)]))
        } else {
            Err(OpossumError::Analysis(
                "source has no light data defined".into(),
            ))
        }
    }
}
impl AnalysisRayTrace for Source {
    fn analyze(
        &mut self,
        _incoming_edges: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        if let Ok(Proptype::LightData(data)) = self.node_attr.get_property("light data") {
            let Some(mut data) = data.value.clone() else {
                return Err(OpossumError::Analysis(
                    "source has empty light data defined".into(),
                ));
            };
            if let LightData::Geometric(rays) = &mut data {
                if let Some(iso) = self.effective_iso() {
                    *rays = rays.transformed_rays(&iso);
                }
                // consider aperture only if not inverted (there is only an output port)
                if !self.inverted() {
                    if let Some(aperture) = self.ports().aperture(&PortType::Output, "out1") {
                        rays.apodize(aperture)?;
                        rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    };
                }
            }
            Ok(LightResult::from([("out1".into(), data)]))
        } else {
            Err(OpossumError::Analysis(
                "source has no light data defined".into(),
            ))
        }
    }
    fn calc_node_position(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let outgoing_edges = AnalysisRayTrace::analyze(self, incoming_data, config)?;
        // generate a single beam (= optical axis) from source
        let mut new_outgoing_edges = LightResult::new();
        for outgoing_edge in &outgoing_edges {
            if let LightData::Geometric(rays) = outgoing_edge.1 {
                let mut axis_ray = if let Ok(Proptype::LengthOption(Some(alignment_wvl))) =
                    self.node_attr.get_property("alignment wavelength")
                {
                    Ray::new_collimated(millimeter!(0.0, 0.0, 0.0), *alignment_wvl, joule!(1.0))
                } else {
                    info!("No alignment wavelength defined, using energy-weighted central wavelength for alignment");
                    rays.get_optical_axis_ray()
                }?;
                if let Some(iso) = self.effective_iso() {
                    axis_ray = axis_ray.transformed_ray(&iso);
                }
                let mut new_rays = Rays::default();
                new_rays.add_ray(axis_ray);
                new_outgoing_edges
                    .insert(outgoing_edge.0.to_string(), LightData::Geometric(new_rays));
            } else {
                return Err(OpossumError::Analysis(
                    "did not receive LightData:Geometric for conversion into OpticalAxis data"
                        .into(),
                ));
            }
        }
        Ok(new_outgoing_edges)
    }
}
impl AnalysisGhostFocus for Source {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        _config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let mut rays = if self.inverted() {
            let Some(bouncing_rays) = incoming_data.get("out1") else {
                return Err(OpossumError::Analysis("no light at port".into()));
            };
            bouncing_rays.clone()
        } else if bounce_lvl == 0 {
            if let Ok(Proptype::LightData(data)) = self.node_attr.get_property("light data") {
                let Some(mut data) = data.value.clone() else {
                    return Err(OpossumError::Analysis(
                        "source has empty light data defined".into(),
                    ));
                };
                if let LightData::Geometric(rays) = &mut data {
                    if let Some(iso) = self.effective_iso() {
                        *rays = rays.transformed_rays(&iso);
                    }
                    vec![rays.clone()]
                } else {
                    return Err(OpossumError::Analysis(
                        "source has wrong light data type defined".into(),
                    ));
                }
            } else {
                return Err(OpossumError::Analysis("could not read light data".into()));
            }
        } else {
            Vec::<Rays>::new()
        };
        if let Some(iso) = self.effective_iso() {
            self.surface.set_isometry(&iso);
            for r in &mut rays {
                r.refract_on_surface(&mut self.surface, None)?;
                self.surface.evaluate_fluence_of_ray_bundle(r)?;
            }
        } else {
            return Err(OpossumError::Analysis(
                "no location for surface defined. Aborting".into(),
            ));
        }

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert("out1".into(), rays);
        Ok(out_light_rays)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{lightdata::DataEnergy, optic_ports::PortType, spectrum_helper::create_he_ne_spec};
    use assert_matches::assert_matches;

    #[test]
    fn default() {
        let mut node = Source::default();
        assert_eq!(node.name(), "source");
        assert_eq!(node.node_type(), "source");
        if let Ok(Proptype::LightData(light_data)) = node.properties().get("light data") {
            assert_eq!(light_data.value, None);
        } else {
            panic!("cannot unpack light data property");
        };
        assert_eq!(node.node_attr().inverted(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", &LightData::Fourier);
        assert_eq!(source.name(), "test");
    }
    #[test]
    fn not_invertable() {
        let mut node = Source::default();
        assert!(node.set_inverted(false).is_ok());
        assert!(node.set_inverted(true).is_ok());
    }
    #[test]
    fn ports() {
        let node = Source::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["in1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["out1"]);
    }
    #[test]
    fn test_set_light_data() {
        let mut src = Source::default();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_eq!(light_data.value, None);
        }
        src.set_light_data(&LightData::Fourier).unwrap();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_matches!(light_data.value.clone().unwrap(), LightData::Fourier);
        }
    }
    #[test]
    fn analyze_no_light_defined() {
        let mut node = Source::default();
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default());
        assert!(output.is_err());
    }
    #[test]
    fn analyze_ok() {
        let light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        let mut node = Source::new("test", &light);
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default()).unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, light);
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", Source::default()), "Source: no data");
        assert_eq!(
            format!("{:?}", Source::new("hallo", &LightData::Fourier)),
            "Source: No display defined for this type of LightData"
        );
    }
}
