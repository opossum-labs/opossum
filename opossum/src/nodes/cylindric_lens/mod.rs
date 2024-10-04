#![warn(missing_docs)]
//! Cylindric lens with spherical or flat surfaces.
use std::collections::HashMap;

use super::node_attr::NodeAttr;
use crate::{
    analyzable::Analyzable,
    analyzers::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    millimeter,
    optic_node::{Alignable, OpticNode},
    optic_ports::{OpticPorts, PortType},
    properties::Proptype,
    rays::Rays,
    refractive_index::{RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{hit_map::HitMap, Cylinder, OpticalSurface, Plane},
    utils::{geom_transformation::Isometry, EnumProxy},
};
#[cfg(feature = "bevy")]
use bevy::{math::primitives::Cuboid, render::mesh::Mesh};
use log::warn;
use num::Zero;
use uom::si::f64::Length;

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

#[derive(Debug)]
/// A real cylindric lens with spherical (or flat) surfaces. By default, the curvature is aligned along the (local) y axis.
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
///   - `front curvature`
///   - `rear curvature`
///   - `center thickness`
///   - `refractive index`
pub struct CylindricLens {
    node_attr: NodeAttr,
    front_surf: OpticalSurface,
    rear_surf: OpticalSurface,
}
impl Default for CylindricLens {
    /// Create a cylindric lens with a center thickness of 10.0 mm. front & back radii of curvature of 500.0 mm and a refractive index of 1.5.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("cylindric lens");
        node_attr
            .create_property(
                "front curvature",
                "radius of curvature of front surface",
                None,
                millimeter!(500.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "rear curvature",
                "radius of curvature of rear surface",
                None,
                millimeter!(-500.0).into(),
            )
            .unwrap();
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

        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "front").unwrap();
        ports.add(&PortType::Output, "rear").unwrap();
        node_attr.set_ports(ports);
        Self {
            node_attr,
            front_surf: OpticalSurface::new(Box::new(
                Cylinder::new(millimeter!(500.0), &Isometry::identity()).unwrap(),
            )),
            rear_surf: OpticalSurface::new(Box::new(
                Cylinder::new(millimeter!(-500.0), &Isometry::identity()).unwrap(),
            )),
        }
    }
}
impl CylindricLens {
    /// Creates a new [`CylindricLens`].
    ///
    /// This function creates a cylindric lens with spherical front and back surfaces, a given center thickness and refractive index.
    /// By default, the curvature aligned along the y axis.

    /// The radii of curvature must not be zero. The given refractive index must not be < 1.0. A radius of curvature of +/- infinity
    /// corresponds to a flat surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given parameters are not correct.
    pub fn new(
        name: &str,
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        refractive_index: &dyn RefractiveIndex,
    ) -> OpmResult<Self> {
        let mut lens = Self::default();
        lens.node_attr.set_name(name);

        if front_curvature.is_zero() || front_curvature.is_nan() {
            return Err(OpossumError::Other(
                "front curvature must not be 0.0 or NaN".into(),
            ));
        }
        lens.node_attr
            .set_property("front curvature", front_curvature.into())?;
        if rear_curvature.is_zero() || rear_curvature.is_nan() {
            return Err(OpossumError::Other(
                "rear curvature must not be 0.0 or NaN".into(),
            ));
        }
        lens.node_attr
            .set_property("rear curvature", rear_curvature.into())?;
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(OpossumError::Other(
                "center thickness must be >= 0.0 and finite".into(),
            ));
        }
        lens.node_attr
            .set_property("center thickness", center_thickness.into())?;

        lens.node_attr.set_property(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        lens.update_surfaces()?;
        Ok(lens)
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let Ok(Proptype::Length(front_roc)) = self.node_attr.get_property("front curvature") else {
            return Err(OpossumError::Analysis("cannot read front curvature".into()));
        };
        self.front_surf = if front_roc.is_infinite() {
            OpticalSurface::new(Box::new(Plane::new(&Isometry::identity())))
        } else {
            OpticalSurface::new(Box::new(Cylinder::new(*front_roc, &Isometry::identity())?))
        };
        let Ok(Proptype::Length(rear_roc)) = self.node_attr.get_property("rear curvature") else {
            return Err(OpossumError::Analysis("cannot read rear curvature".into()));
        };
        self.rear_surf = if rear_roc.is_infinite() {
            OpticalSurface::new(Box::new(Plane::new(&Isometry::identity())))
        } else {
            OpticalSurface::new(Box::new(Cylinder::new(*rear_roc, &Isometry::identity())?))
        };
        Ok(())
    }
    fn analyze_forward(
        &mut self,
        incoming_rays: Rays,
        thickness: Length,
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
        let isometry = iso.append(&thickness_iso);
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
        let isometry = iso.append(&thickness_iso);
        self.rear_surf.set_isometry(&isometry);
        self.rear_surf.set_coating(
            self.node_attr()
                .ports()
                .coating(&PortType::Output, "rear")
                .unwrap()
                .clone(),
        );
        if let Some(aperture) = self.ports().aperture(&PortType::Output, "front") {
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

        if let Some(aperture) = self.ports().aperture(&PortType::Input, "rear") {
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

impl OpticNode for CylindricLens {
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
    fn after_deserialization_hook(&mut self) -> OpmResult<()> {
        self.update_surfaces()
    }
}

impl Alignable for CylindricLens {}

impl Dottable for CylindricLens {
    fn node_color(&self) -> &str {
        "aqua"
    }
}
impl Analyzable for CylindricLens {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{energy::AnalysisEnergy, raytrace::AnalysisRayTrace, RayTraceConfig},
        joule,
        light_result::LightResult,
        lightdata::LightData,
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        position_distributions::Hexapolar,
        properties::Proptype,
        rays::Rays,
    };
    use approx::assert_relative_eq;
    use nalgebra::Vector3;
    #[test]
    fn default() {
        let mut node = CylindricLens::default();
        assert_eq!(node.name(), "cylindric lens");
        assert_eq!(node.node_type(), "cylindric lens");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "aqua");
        assert!(node.as_group().is_err());
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(500.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(-500.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(10.0));
        let Ok(Proptype::RefractiveIndex(index)) = node.node_attr.get_property("refractive index")
        else {
            panic!()
        };
        assert_eq!(
            (*index).value.get_refractive_index(Length::zero()).unwrap(),
            1.5
        );
    }
    #[test]
    fn new() {
        let roc = millimeter!(100.0);
        let ct = millimeter!(11.0);
        let ref_index = RefrIndexConst::new(1.5).unwrap();

        assert!(CylindricLens::new("test", roc, roc, millimeter!(-0.1), &ref_index).is_err());
        assert!(CylindricLens::new("test", roc, roc, millimeter!(f64::NAN), &ref_index).is_err());
        assert!(
            CylindricLens::new("test", roc, roc, millimeter!(f64::INFINITY), &ref_index).is_err()
        );

        assert!(CylindricLens::new("test", roc, Length::zero(), ct, &ref_index).is_err());
        assert!(CylindricLens::new("test", roc, millimeter!(f64::NAN), ct, &ref_index).is_err());
        assert!(
            CylindricLens::new("test", roc, millimeter!(f64::INFINITY), ct, &ref_index).is_ok()
        );
        assert!(
            CylindricLens::new("test", roc, millimeter!(f64::NEG_INFINITY), ct, &ref_index).is_ok()
        );

        assert!(CylindricLens::new("test", Length::zero(), roc, ct, &ref_index).is_err());
        assert!(CylindricLens::new("test", millimeter!(f64::NAN), roc, ct, &ref_index).is_err());
        assert!(
            CylindricLens::new("test", millimeter!(f64::INFINITY), roc, ct, &ref_index).is_ok()
        );
        assert!(
            CylindricLens::new("test", millimeter!(f64::NEG_INFINITY), roc, ct, &ref_index).is_ok()
        );
        let ref_index = RefrIndexConst::new(2.0).unwrap();
        let node = CylindricLens::new("test", roc, roc, ct, &ref_index).unwrap();
        assert_eq!(node.name(), "test");
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(roc)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(11.0));
        let Ok(Proptype::RefractiveIndex(EnumProxy::<RefractiveIndexType> {
            value: RefractiveIndexType::Const(ref_index_const),
        })) = node.node_attr.get_property("refractive index")
        else {
            panic!()
        };
        assert_eq!(
            (*ref_index_const)
                .get_refractive_index(Length::zero())
                .unwrap(),
            2.0
        );
    }
    #[test]
    fn inverted() {
        test_inverted::<CylindricLens>()
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<CylindricLens>()
    }
    #[test]
    fn analyze_wrong_port() {
        let mut node = CylindricLens::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("rear".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<CylindricLens>("front");
    }
    #[test]
    fn analyze_flatflat() {
        let mut node = CylindricLens::new(
            "test",
            millimeter!(f64::INFINITY),
            millimeter!(f64::NEG_INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(2.0).unwrap(),
        )
        .unwrap();
        node.set_isometry(Isometry::new_along_z(millimeter!(10.0)).unwrap());
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let mut incoming_data = LightResult::default();
        incoming_data.insert("front".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())
                .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("rear") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
                assert_eq!(ray.path_length(), millimeter!(30.0));
            }
        } else {
            assert!(false);
        }
    }
    #[test]
    fn analyze_biconvex() {
        // biconvex lens with index of 1.0 (="neutral" lens)
        let mut node = CylindricLens::new(
            "test",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0).unwrap(),
        )
        .unwrap();
        node.set_isometry(Isometry::identity());
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let mut incoming_data = LightResult::default();
        incoming_data.insert("front".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())
                .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("rear") {
            for ray in rays {
                assert_eq!(ray.direction().x, 0.0);
                assert_eq!(ray.direction().y, 0.0);
                assert_relative_eq!(ray.direction().z, 1.0);
            }
        } else {
            assert!(false);
        }
    }
}
