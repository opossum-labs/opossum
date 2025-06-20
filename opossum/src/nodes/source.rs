#![warn(missing_docs)]
use log::{info, warn};
use opm_macros_lib::OpmNode;
use uom::si::f64::Length;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        GhostFocusConfig, RayTraceConfig,
        energy::AnalysisEnergy,
        ghostfocus::AnalysisGhostFocus,
        raytrace::{AnalysisRayTrace, MissedSurfaceStrategy},
    },
    error::{OpmResult, OpossumError},
    joule,
    light_result::{LightRays, LightResult},
    lightdata::{LightData, light_data_builder::LightDataBuilder},
    millimeter,
    optic_node::OpticNode,
    optic_ports::PortType,
    properties::Proptype,
    ray::Ray,
    rays::Rays,
    utils::geom_transformation::Isometry,
};
use std::fmt::Debug;

/// A general light source
///
/// Hence it has only one output port (out1) and effectively no input ports. The formal input port `in1` is discarded during analysis.
/// Source nodes usually are the first nodes of a [`NodeGroup`](crate::nodes::NodeGroup).
///
/// ## Optical Ports
///   - Inputs
///     - `input_1` (input discarded, used to make the node invertable)
///   - Outputs
///     - `output_1`
///
/// ## Properties
///   - `light data`
///   - `light data iso`
///   - `alignment wavelength`
///
/// **Note**: If a [`Source`] is configured as `inverted` the initial output port becomes an input port and further data is discarded.
#[derive(OpmNode, Clone)]
#[opm_node("slateblue")]
pub struct Source {
    node_attr: NodeAttr,
}
unsafe impl Send for Source {}

impl Default for Source {
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("source");
        node_attr
            .create_property(
                "light data",
                "data of the emitted light",
                Option::<LightDataBuilder>::None.into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "light data iso",
                "isometry of the emitted light field",
                Option::<Isometry>::None.into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "alignment wavelength",
                "wavelength to be used for alignment. Necessary for, e.g., grating alignments",
                Proptype::LengthOption(None),
            )
            .unwrap();

        let mut src = Self { node_attr };
        src.update_surfaces().unwrap();
        src
    }
}
impl Source {
    /// Creates a new [`Source`].
    ///
    /// The light to be emitted from this source is defined in a [`LightData`] structure.
    ///
    /// # Panics
    /// Panics if [`Properties`](crate::properties::Properties) `light data` can not be set
    ///
    /// ## Example
    ///
    /// ```rust
    /// use opossum::{
    /// lightdata::{light_data_builder::LightDataBuilder, energy_data_builder::EnergyDataBuilder},
    /// nodes::Source,
    /// spectrum_helper::create_he_ne_spec};
    ///
    /// let light_data_builder = LightDataBuilder::Energy(EnergyDataBuilder::Raw(create_he_ne_spec(1.0).unwrap()));
    /// let source=Source::new("My Source", light_data_builder);
    /// ```
    #[must_use]
    pub fn new(name: &str, light_data_builder: LightDataBuilder) -> Self {
        let mut source = Self::default();
        source.node_attr.set_name(name);
        source
            .node_attr
            .set_property("light data", Some(light_data_builder).into())
            .unwrap();
        source.update_surfaces().unwrap();
        source
    }

    /// Sets the alignment wavelength for an optical scenery
    /// This function is useful, or example, when aligning grating setups that should be analyzed with a given spectrum,
    /// but should be positioned to be ideal for a certain wavelength
    /// # Errors
    /// This function only propagates the errors of the contained functions
    pub fn set_alignment_wavelength(&mut self, wvl: Length) -> OpmResult<()> {
        if wvl.is_sign_negative() || !wvl.is_normal() {
            return Err(OpossumError::Other(
                "wavelength must be positive and finite".into(),
            ));
        }
        self.node_attr
            .set_property("alignment wavelength", Proptype::LengthOption(Some(wvl)))
    }

    /// Sets the light data builder of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    ///
    /// # Attributes
    /// * `light_data_builder`: [`LightDataBuilder`] that shall be set
    ///
    /// # Errors
    /// This function returns an error if the property "light data" can not be set
    pub fn set_light_data(&mut self, light_data_builder: LightDataBuilder) -> OpmResult<()> {
        self.node_attr
            .set_property("light data", Some(light_data_builder).into())?;
        Ok(())
    }
}
impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.node_attr.get_property("light data").unwrap();
        let data = if let Proptype::LightDataBuilder(data) = &light_prop {
            data
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
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
}
impl AnalysisEnergy for Source {
    fn analyze(&mut self, _incoming_data: LightResult) -> OpmResult<LightResult> {
        if let Ok(Proptype::LightDataBuilder(light_data_builder)) =
            self.node_attr.get_property("light data")
        {
            let data = if let Some(light_data_builder) = light_data_builder.clone() {
                light_data_builder.build()?
            } else {
                return Err(OpossumError::Analysis(
                    "source has empty light data builder".into(),
                ));
            };
            Ok(LightResult::from([("output_1".into(), data)]))
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
        if let Ok(Proptype::LightDataBuilder(light_data_builder)) =
            self.node_attr.get_property("light data")
        {
            let mut data = if let Some(lightdata_builder) = light_data_builder.clone() {
                lightdata_builder.build()?
            } else {
                return Err(OpossumError::Analysis(
                    "source has empty light data builder".into(),
                ));
            };
            if let LightData::Geometric(rays) = &mut data {
                if let Ok(Proptype::Isometry(Some(iso))) =
                    self.node_attr.get_property("light data iso")
                {
                    *rays = rays.transformed_by_iso(iso);
                }
                if let Ok(iso) = self.effective_surface_iso("input_1") {
                    *rays = rays.transformed_by_iso(&iso);
                    // consider aperture only if not inverted (there is only an output port)
                    if !self.inverted() {
                        match self.ports().aperture(&PortType::Output, "output_1") {
                            Some(aperture) => {
                                rays.apodize(aperture, &iso)?;
                                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                            }
                            _ => {
                                return Err(OpossumError::OpticPort(
                                    "output aperture not found".into(),
                                ));
                            }
                        }
                    }
                }
            }
            Ok(LightResult::from([("output_1".into(), data)]))
        } else {
            Err(OpossumError::Analysis(
                "source has no light data defined".into(),
            ))
        }
    }
    fn calc_node_positions(
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
                    info!(
                        "No alignment wavelength defined, using energy-weighted central wavelength for alignment"
                    );
                    rays.get_optical_axis_ray()
                }?;
                let iso = self.effective_surface_iso("input_1")?;
                axis_ray = axis_ray.transformed_ray(&iso);

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
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let mut rays = if self.inverted() {
            let Some(bouncing_rays) = incoming_data.get("output_1") else {
                return Err(OpossumError::Analysis("no light at port".into()));
            };
            bouncing_rays.clone()
        } else if bounce_lvl == 0 {
            if let Ok(Proptype::LightDataBuilder(light_data_builder)) =
                self.node_attr.get_property("light data")
            {
                let mut data = if let Some(lightdata_builder) = light_data_builder.clone() {
                    lightdata_builder.build()?
                } else {
                    return Err(OpossumError::Analysis(
                        "source has empty light data builder".into(),
                    ));
                };
                if let LightData::Geometric(rays) = &mut data {
                    if let Ok(Proptype::Isometry(Some(iso))) =
                        self.node_attr.get_property("light data iso")
                    {
                        *rays = rays.transformed_by_iso(iso);
                    }
                    let iso = self.effective_surface_iso("output_1")?;
                    *rays = rays.transformed_by_iso(&iso);

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

        if let Some(surf) = self.get_optic_surface_mut("input_1") {
            let refraction_intended = true;
            for r in &mut rays {
                r.refract_on_surface(
                    surf,
                    None,
                    refraction_intended,
                    &MissedSurfaceStrategy::Ignore,
                )?;
                surf.evaluate_fluence_of_ray_bundle(r, config.fluence_estimator())?;
            }
        } else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        }

        let mut out_light_rays = LightRays::default();
        out_light_rays.insert("output_1".into(), rays);
        Ok(out_light_rays)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        nanometer, optic_ports::PortType, position_distributions::Hexapolar,
        spectrum_helper::create_he_ne_spec, utils::geom_transformation::Isometry,
    };
    use assert_matches::assert_matches;
    use core::f64;

    #[test]
    fn default() {
        let mut node = Source::default();
        assert_eq!(node.name(), "source");
        assert_eq!(node.node_type(), "source");
        if let Proptype::LightDataBuilder(light_data) = node.properties().get("light data").unwrap()
        {
            assert!(light_data.is_none());
        } else {
            panic!("wrong type for `light data` property");
        };
        if let Proptype::Isometry(iso) = node.properties().get("light data iso").unwrap() {
            assert!(iso.is_none());
        } else {
            panic!("wrong type for `light data iso` property");
        };
        if let Proptype::LengthOption(wvl) = node.properties().get("alignment wavelength").unwrap()
        {
            assert!(wvl.is_none());
        } else {
            panic!("wrong type for `alignment wavelength` property");
        };
        assert_eq!(node.node_attr().inverted(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", LightDataBuilder::Fourier);
        assert_eq!(source.name(), "test");
    }
    #[test]
    fn set_alignment_wavelength() {
        let mut node = Source::default();
        assert!(node.set_alignment_wavelength(nanometer!(0.0)).is_err());
        assert!(node.set_alignment_wavelength(nanometer!(f64::NAN)).is_err());
        assert!(
            node.set_alignment_wavelength(nanometer!(f64::INFINITY))
                .is_err()
        );
        assert!(
            node.set_alignment_wavelength(nanometer!(f64::NEG_INFINITY))
                .is_err()
        );
        assert!(node.set_alignment_wavelength(nanometer!(-0.1)).is_err());
        assert!(node.set_alignment_wavelength(nanometer!(600.0)).is_ok());
        let Proptype::LengthOption(wavelength) =
            node.node_attr.get_property("alignment wavelength").unwrap()
        else {
            panic!("wrong proptype")
        };
        assert_eq!(wavelength, &Some(nanometer!(600.0)));
    }
    #[test]
    fn set_property() {
        let mut node = Source::default();
        node.set_property(
            "alignment wavelength",
            Proptype::LengthOption(Some(nanometer!(600.0))),
        )
        .unwrap();
        let Proptype::LengthOption(wavelength) =
            node.node_attr.get_property("alignment wavelength").unwrap()
        else {
            panic!("wrong proptype")
        };
        assert_eq!(wavelength, &Some(nanometer!(600.0)));
    }
    #[test]
    fn is_invertable() {
        let mut node = Source::default();
        assert!(node.set_inverted(false).is_ok());
        assert!(node.set_inverted(true).is_ok());
    }
    #[test]
    fn ports() {
        let node = Source::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn test_set_light_data() {
        let mut src = Source::default();
        if let Proptype::LightDataBuilder(light_data) = src.properties().get("light data").unwrap()
        {
            assert!(light_data.is_none());
        }
        src.set_light_data(LightDataBuilder::Fourier).unwrap();
        if let Proptype::LightDataBuilder(light_data) = src.properties().get("light data").unwrap()
        {
            assert_matches!(light_data.clone().unwrap(), LightDataBuilder::Fourier);
        }
    }
    #[test]
    fn analyze_energy_no_light_defined() {
        let mut node = Source::default();
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default());
        assert!(output.is_err());
    }
    #[test]
    fn analyze_energy_ok() {
        let light_builder = LightDataBuilder::Energy(create_he_ne_spec(1.0).unwrap().into());
        let mut node = Source::new("test", light_builder.clone());
        let output = AnalysisEnergy::analyze(&mut node, LightResult::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, light_builder.build().unwrap());
    }
    #[test]
    fn analyze_raytrace_no_light_defined() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let output = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::default(),
            &RayTraceConfig::default(),
        );
        assert_eq!(
            output.unwrap_err(),
            OpossumError::Analysis("source has empty light data builder".into())
        );
    }
    #[test]
    fn analyze_raytrace_ok() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.0), 0).unwrap(),
        )
        .unwrap();
        let light_data_builder = LightDataBuilder::Geometric(rays.into());
        node.set_light_data(light_data_builder).unwrap();
        let output = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::default(),
            &RayTraceConfig::default(),
        )
        .unwrap();
        let light_data = output.get("output_1").unwrap();
        if let LightData::Geometric(rays) = light_data {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.wavelength(), nanometer!(1000.0));
            assert_eq!(ray.position().x, millimeter!(0.0));
            assert_eq!(ray.position().y, millimeter!(0.0));
            assert_eq!(ray.position().z, millimeter!(0.0));
        } else {
            panic!("no geometric light data found")
        }
    }
    #[test]
    fn analyze_raytrace_light_data_iso() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.0), 0).unwrap(),
        )
        .unwrap();
        let light_data_builder = LightDataBuilder::Geometric(rays.into());
        node.set_light_data(light_data_builder).unwrap();
        let light_iso = Isometry::new_translation(millimeter!(0.0, 1.0, 0.0)).unwrap();
        node.set_property("light data iso", Some(light_iso).into())
            .unwrap();
        let output = AnalysisRayTrace::analyze(
            &mut node,
            LightResult::default(),
            &RayTraceConfig::default(),
        )
        .unwrap();
        let light_data = output.get("output_1").unwrap();
        if let LightData::Geometric(rays) = light_data {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.wavelength(), nanometer!(1000.0));
            assert_eq!(ray.position().x, millimeter!(0.0));
            assert_eq!(ray.position().y, millimeter!(1.0));
            assert_eq!(ray.position().z, millimeter!(0.0));
        } else {
            panic!("no geometric light data found")
        }
    }
    #[test]
    fn calc_node_position_ok_alignement_wavelength_set() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        node.set_alignment_wavelength(nanometer!(630.0)).unwrap();
        let light_data_builder = LightDataBuilder::Geometric(Rays::default().into());
        node.set_light_data(light_data_builder).unwrap();
        let output = AnalysisRayTrace::calc_node_positions(
            &mut node,
            LightResult::default(),
            &RayTraceConfig::default(),
        )
        .unwrap();
        let light_data = output.get("output_1").unwrap();
        if let LightData::Geometric(rays) = light_data {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.wavelength(), nanometer!(630.0));
        } else {
            panic!("no geometric light data found")
        }
    }
    #[test]
    fn analyze_ghost_focus_no_light_defined() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::default(),
            &GhostFocusConfig::default(),
            &mut vec![],
            0,
        );
        assert_eq!(
            output.unwrap_err(),
            OpossumError::Analysis("source has empty light data builder".into())
        );
    }
    #[test]
    fn analyze_ghost_focus_ok() {
        let mut node = Source::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(1.0), 1).unwrap(),
        )
        .unwrap();
        let light_data_builder = LightDataBuilder::Geometric(rays.clone().into());
        node.set_light_data(light_data_builder).unwrap();
        let mut light_rays = LightRays::new();
        light_rays.insert("input_1".into(), vec![rays]);
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            light_rays,
            &GhostFocusConfig::default(),
            &mut vec![],
            0,
        );
        assert!(output.is_ok());
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", Source::default()), "Source: no data");
        assert_eq!(
            format!("{:?}", Source::new("hallo", LightDataBuilder::Fourier)),
            "Source: Fourier"
        );
    }
}
