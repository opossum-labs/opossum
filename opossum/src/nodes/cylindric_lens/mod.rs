#![warn(missing_docs)]
//! Cylindric lens with spherical or flat surfaces.

use std::{cell::RefCell, rc::Rc};

use super::node_attr::NodeAttr;
use crate::{
    analyzers::Analyzable,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    meter, millimeter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::Proptype,
    radian,
    refractive_index::{RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{geo_surface::GeoSurfaceRef, Cylinder, Plane},
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

#[derive(Debug, Clone)]
/// A real cylindric lens with spherical (or flat) surfaces. By default, the curvature is aligned along the (local) y axis.
///
/// # Curvature convention:
/// - negative curvature on the input will be a concave (defocusing) surface
/// - positive curvature on the input will be a convex (focusing) surface
/// - negative curvature on the output will be a convex (focusing) surface
/// - positive curvature on the output will be a concave (defocusing) surface
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
}
impl Default for CylindricLens {
    /// Create a cylindric lens with a center thickness of 10.0 mm. front & back radii of curvature of 500.0 mm and a refractive index of 1.5.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("cylindric lens");
        node_attr
            .create_property(
                "front curvature",
                "radius of curvature of front surface",
                millimeter!(500.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "rear curvature",
                "radius of curvature of rear surface",
                millimeter!(-500.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "center thickness",
                "thickness of the lens in the center",
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the lens material",
                EnumProxy::<RefractiveIndexType> {
                    value: RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()),
                }
                .into(),
            )
            .unwrap();
        let mut cyl_lens = Self { node_attr };
        cyl_lens.update_surfaces().unwrap();
        cyl_lens
    }
}
impl CylindricLens {
    /// Creates a new [`CylindricLens`].
    ///
    /// This function creates a cylindric lens with spherical front and back surfaces, a given center thickness and refractive index.
    /// By default, the curvature aligned along the y axis.
    ///
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
        let mut cyl_lens = Self::default();
        cyl_lens.node_attr.set_name(name);

        if front_curvature.is_zero() || front_curvature.is_nan() {
            return Err(OpossumError::Other(
                "front curvature must not be 0.0 or NaN".into(),
            ));
        }
        cyl_lens
            .node_attr
            .set_property("front curvature", front_curvature.into())?;
        if rear_curvature.is_zero() || rear_curvature.is_nan() {
            return Err(OpossumError::Other(
                "rear curvature must not be 0.0 or NaN".into(),
            ));
        }
        cyl_lens
            .node_attr
            .set_property("rear curvature", rear_curvature.into())?;
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(OpossumError::Other(
                "center thickness must be >= 0.0 and finite".into(),
            ));
        }
        cyl_lens
            .node_attr
            .set_property("center thickness", center_thickness.into())?;

        cyl_lens.node_attr.set_property(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        cyl_lens.update_surfaces()?;
        Ok(cyl_lens)
    }
}

impl OpticNode for CylindricLens {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let Ok(Proptype::Length(front_curvature)) = self.node_attr.get_property("front curvature")
        else {
            return Err(OpossumError::Analysis("cannot read front curvature".into()));
        };
        let (front_geosurface, anchor_point_iso_front) = if front_curvature.is_infinite() {
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Plane::new(node_iso.clone())))),
                Isometry::identity(),
            )
        } else {
            let anchor_point_iso_front =
                Isometry::new(meter!(0., 0., front_curvature.value), radian!(0., 0., 0.))?;
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Cylinder::new(
                    *front_curvature,
                    node_iso.append(&anchor_point_iso_front),
                )?))),
                anchor_point_iso_front,
            )
        };
        self.update_surface(
            &"input_1".to_string(),
            front_geosurface,
            anchor_point_iso_front,
            &PortType::Input,
        )?;
        let Ok(Proptype::Length(rear_curvature)) = self.node_attr.get_property("rear curvature")
        else {
            return Err(OpossumError::Analysis("cannot read rear curvature".into()));
        };
        let Ok(Proptype::Length(center_thickness)) =
            self.node_attr.get_property("center thickness")
        else {
            return Err(OpossumError::Analysis(
                "cannot read center thickness".into(),
            ));
        };
        let (rear_geosurface, anchor_point_iso_rear) = if rear_curvature.is_infinite() {
            let anchor_point_iso_rear =
                Isometry::new(meter!(0., 0., center_thickness.value), radian!(0., 0., 0.))?;
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Plane::new(
                    node_iso.append(&anchor_point_iso_rear),
                )))),
                anchor_point_iso_rear,
            )
        } else {
            let anchor_point_iso_rear = Isometry::new(
                meter!(0., 0., (*rear_curvature + *center_thickness).value),
                radian!(0., 0., 0.),
            )?;
            (
                GeoSurfaceRef(Rc::new(RefCell::new(Cylinder::new(
                    *rear_curvature,
                    node_iso.append(&anchor_point_iso_rear),
                )?))),
                anchor_point_iso_rear,
            )
        };
        self.update_surface(
            &"output_1".to_string(),
            rear_geosurface,
            anchor_point_iso_rear,
            &PortType::Output,
        )
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    ///updates the lidt of the optical surfaces after deserialization
    fn update_lidt(&mut self) -> OpmResult<()> {
        let lidt = *self.node_attr().lidt();
        let in_ports = self.ports().names(&PortType::Input);
        let out_ports = self.ports().names(&PortType::Output);

        for port_name in &in_ports {
            if let Some(opt_surf) = self.get_optic_surface_mut(port_name) {
                opt_surf.set_lidt(lidt)?;
            }
        }
        for port_name in &out_ports {
            if let Some(opt_surf) = self.get_optic_surface_mut(port_name) {
                opt_surf.set_lidt(lidt)?;
            }
        }
        Ok(())
    }
}

impl Alignable for CylindricLens {}
impl Dottable for CylindricLens {
    fn node_color(&self) -> &str {
        "aqua"
    }
}
impl LIDT for CylindricLens {}
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
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<CylindricLens>("input_1");
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
        node.set_isometry(Isometry::new_along_z(millimeter!(10.0)).unwrap())
            .unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())
                .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
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
        node.set_isometry(Isometry::identity()).unwrap();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())
                .unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            for ray in rays {
                assert_relative_eq!(ray.direction().x, 0.0);
                assert_relative_eq!(ray.direction().y, 0.0);
                assert_relative_eq!(ray.direction().z, 1.0);
            }
        } else {
            assert!(false);
        }
    }
}
