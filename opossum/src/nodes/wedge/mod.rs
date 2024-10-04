use std::collections::HashMap;

use super::NodeAttr;
use crate::{
    analyzable::Analyzable,
    analyzers::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    millimeter,
    optic_node::{Alignable, OpticNode},
    optic_ports::{OpticPorts, PortType},
    rays::Rays,
    refractive_index::{RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{hit_map::HitMap, OpticalSurface, Plane},
    utils::{geom_transformation::Isometry, EnumProxy},
};
use nalgebra::Point3;
use num::Zero;
use uom::si::{
    angle::degree,
    f64::{Angle, Length},
};

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

#[derive(Debug)]
/// An optical element with two flat surfaces, a given thickness and a  given wedge angle (= wedged window).
///
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `center thickness`
///   - `refractive index`
///   - `wedge`
pub struct Wedge {
    node_attr: NodeAttr,
    front_surf: OpticalSurface,
    rear_surf: OpticalSurface,
}
impl Default for Wedge {
    /// Create a wedge with a center thickness of 10.0 mm, refractive index of 1.5 and no wedge angle (flat windows)
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("wedge");
        node_attr
            .create_property(
                "center thickness",
                "thickness of the lens in the center",
                None,
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the lens material",
                None,
                EnumProxy::<RefractiveIndexType> {
                    value: RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()),
                }
                .into(),
            )
            .unwrap();
        node_attr
            .create_property("wedge", "wedge angle", None, Angle::zero().into())
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "front").unwrap();
        ports.add(&PortType::Output, "rear").unwrap();
        node_attr.set_ports(ports);
        Self {
            node_attr,
            front_surf: OpticalSurface::new(Box::new(Plane::new(&Isometry::identity()))),
            rear_surf: OpticalSurface::new(Box::new(Plane::new(&Isometry::identity()))),
        }
    }
}
impl Wedge {
    /// Create a new wedge.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the center thickness is ngeative or not finite
    ///   - the wedge angle is outside ]-90°; 90°[ or not finite
    pub fn new(
        name: &str,
        center_thickness: Length,
        wedge_angle: Angle,
        refractive_index: &dyn RefractiveIndex,
    ) -> OpmResult<Self> {
        let mut wedge = Self::default();
        wedge.node_attr.set_name(name);
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(crate::error::OpossumError::Other(
                "center thickness must be positive and finite".into(),
            ));
        }
        wedge
            .node_attr
            .set_property("center thickness", center_thickness.into())?;

        wedge.node_attr.set_property(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        if !wedge_angle.is_finite() || wedge_angle.get::<degree>().abs() > 90.0 {
            return Err(crate::error::OpossumError::Other(
                "wedge angle must be within the interval ]-90 deg; 90 deg[ and finite".into(),
            ));
        }
        wedge.node_attr.set_property("wedge", wedge_angle.into())?;
        Ok(wedge)
    }
    fn analyze_forward(
        &mut self,
        incoming_rays: Rays,
        thickness: Length,
        wedge: Angle,
        refri: &RefractiveIndexType,
        iso: &Isometry,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<Rays> {
        let ambient_idx = self.ambient_idx();
        let mut rays = incoming_rays;
        self.front_surf.set_isometry(iso);
        self.front_surf.set_coating(
            self.node_attr()
                .ports()
                .coating(&PortType::Input, "front")
                .unwrap()
                .clone(),
        );
        let thickness_iso = Isometry::new_along_z(thickness)?;
        let wedge_iso = Isometry::new(
            Point3::origin(),
            Point3::new(wedge, Angle::zero(), Angle::zero()),
        )?;
        let isometry = iso.append(&thickness_iso).append(&wedge_iso);
        self.rear_surf.set_isometry(&isometry);
        self.rear_surf.set_coating(
            self.node_attr()
                .ports()
                .coating(&PortType::Output, "rear")
                .unwrap()
                .clone(),
        );
        if let Some(aperture) = self.ports().aperture(&PortType::Input, "front") {
            rays.apodize(aperture)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("input aperture not found".into()));
        };
        let reflected_front = rays.refract_on_surface(&mut self.front_surf, Some(refri))?;
        self.front_surf.set_backwards_rays_cache(reflected_front);
        rays.merge(self.front_surf.forward_rays_cache());
        rays.set_refractive_index(refri)?;
        let reflected_rear = rays.refract_on_surface(&mut self.rear_surf, Some(&ambient_idx))?;
        self.rear_surf.set_backwards_rays_cache(reflected_rear);
        rays.merge(self.rear_surf.forward_rays_cache());
        if let Some(aperture) = self.ports().aperture(&PortType::Output, "rear") {
            rays.apodize(aperture)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("output aperture not found".into()));
        };
        Ok(rays)
    }
    fn analyze_inverse(
        &mut self,
        incoming_rays: Rays,
        thickness: Length,
        wedge: Angle,
        refri: &RefractiveIndexType,
        iso: &Isometry,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<Rays> {
        let ambient_idx = self.ambient_idx();
        let mut rays = incoming_rays;
        self.front_surf.set_isometry(iso);
        let thickness_iso = Isometry::new_along_z(thickness)?;
        let wedge_iso = Isometry::new(
            Point3::origin(),
            Point3::new(wedge, Angle::zero(), Angle::zero()),
        )?;
        let isometry = iso.append(&thickness_iso).append(&wedge_iso);
        self.rear_surf.set_isometry(&isometry);
        if let Some(aperture) = self.ports().aperture(&PortType::Input, "rear") {
            rays.apodize(aperture)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("output aperture not found".into()));
        };
        let reflected_rear = rays.refract_on_surface(&mut self.rear_surf, Some(refri))?;
        self.rear_surf.set_forward_rays_cache(reflected_rear);
        rays.merge(self.rear_surf.backwards_rays_cache());
        rays.set_refractive_index(refri)?;

        let reflected_front = rays.refract_on_surface(&mut self.front_surf, Some(&ambient_idx))?;
        self.front_surf.set_forward_rays_cache(reflected_front);
        rays.merge(self.front_surf.backwards_rays_cache());
        if let Some(aperture) = self.ports().aperture(&PortType::Output, "front") {
            rays.apodize(aperture)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("input aperture not found".into()));
        };
        Ok(rays)
    }
}

impl OpticNode for Wedge {
    fn reset_data(&mut self) {
        self.front_surf.set_backwards_rays_cache(Rays::default());
        self.front_surf.set_forward_rays_cache(Rays::default());

        self.rear_surf.set_backwards_rays_cache(Rays::default());
        self.rear_surf.set_forward_rays_cache(Rays::default());
    }
    fn hit_maps(&self) -> HashMap<String, HitMap> {
        let mut map: HashMap<String, HitMap> = HashMap::default();
        map.insert("front".to_string(), self.front_surf.hit_map().to_owned());
        map.insert("rear".to_string(), self.rear_surf.hit_map().to_owned());
        map
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
}

impl Alignable for Wedge {}

impl Dottable for Wedge {
    fn node_color(&self) -> &str {
        "aquamarine"
    }
}
impl Analyzable for Wedge {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{energy::AnalysisEnergy, raytrace::AnalysisRayTrace, RayTraceConfig},
        degree, joule,
        light_result::LightResult,
        lightdata::{DataEnergy, LightData},
        nanometer,
        nodes::test_helper::test_helper::*,
        optic_ports::PortType,
        properties::Proptype,
        ray::Ray,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
    };
    use nalgebra::Vector3;

    #[test]
    fn default() {
        let node = Wedge::default();
        assert_eq!(node.name(), "wedge");
        assert_eq!(node.node_type(), "wedge");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.node_color(), "aquamarine");
        assert_eq!(node.inverted(), false);
        if let Ok(Proptype::Length(p)) = node.properties().get("center thickness") {
            assert_eq!(p, &millimeter!(10.0));
        } else {
            assert!(false, "could not read center thickness.");
        }
        if let Ok(Proptype::Angle(p)) = node.properties().get("wedge") {
            assert_eq!(p, &degree!(0.0));
        } else {
            assert!(false, "could not read angle.");
        }
        if let Ok(Proptype::RefractiveIndex(p)) = node.properties().get("refractive index") {
            if let RefractiveIndexType::Const(val) = &p.value {
                let idx = val.get_refractive_index(nanometer!(1000.0)).unwrap();
                assert_eq!(idx, 1.5);
            } else {
                assert!(false, "could not read refractive index constant.");
            }
        } else {
            assert!(false, "could not read refractive index.");
        }
    }
    #[test]
    fn new() {
        assert!(Wedge::new(
            "test",
            millimeter!(-0.1),
            degree!(0.0),
            &RefrIndexConst::new(1.5).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(f64::NEG_INFINITY),
            degree!(0.0),
            &RefrIndexConst::new(1.5).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(f64::INFINITY),
            degree!(0.0),
            &RefrIndexConst::new(1.5).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(f64::NAN),
            degree!(0.0),
            &RefrIndexConst::new(1.5).unwrap()
        )
        .is_err());

        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(f64::NEG_INFINITY),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(f64::INFINITY),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(f64::NAN),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(90.01),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(-90.01),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_err());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(89.99),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_ok());
        assert!(Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(-89.99),
            &RefrIndexConst::new(1.0).unwrap()
        )
        .is_ok());
        let n = Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(10.0),
            &RefrIndexConst::new(1.0).unwrap(),
        )
        .unwrap();
        assert_eq!(n.name(), "test");
        if let Ok(Proptype::Length(p)) = n.properties().get("center thickness") {
            assert_eq!(p, &millimeter!(0.0));
        } else {
            assert!(false, "could not read center thickness.");
        }
        if let Ok(Proptype::Angle(p)) = n.properties().get("wedge") {
            assert_eq!(p, &degree!(10.0));
        } else {
            assert!(false, "could not read angle.");
        }
        if let Ok(Proptype::RefractiveIndex(p)) = n.properties().get("refractive index") {
            if let RefractiveIndexType::Const(val) = &p.value {
                let idx = val.get_refractive_index(nanometer!(1000.0)).unwrap();
                assert_eq!(idx, 1.0);
            } else {
                assert!(false, "could not read refractive index constant.");
            }
        } else {
            assert!(false, "could not read refractive index.");
        }
    }
    #[test]
    fn ports() {
        let node = Wedge::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["front"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["rear"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<Wedge>("front", "rear");
    }
    #[test]
    fn inverted() {
        test_inverted::<Wedge>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<Wedge>()
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = Wedge::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("rear".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = Wedge::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("front".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("rear"));
        assert_eq!(output.len(), 1);
        let output = output.get("rear");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<Wedge>("front");
    }
    #[test]
    fn analyze_geometric_ok() {
        let mut node = Wedge::default();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        );
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("front".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("rear") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 20.0));
            let dir = Vector3::new(0.0_f64, 0.0, 1.0);
            assert_eq!(ray.direction(), dir);
        } else {
            assert!(false, "could not get LightData");
        }
    }
}
