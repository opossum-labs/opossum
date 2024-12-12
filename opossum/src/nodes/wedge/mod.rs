use std::{cell::RefCell, rc::Rc};

use super::NodeAttr;
use crate::{
    analyzers::Analyzable,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    millimeter,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::Proptype,
    refractive_index::{RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::{geo_surface::GeoSurfaceRef, Plane},
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

#[derive(Debug, Clone)]
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
}
impl Default for Wedge {
    /// Create a wedge with a center thickness of 10.0 mm, refractive index of 1.5 and no wedge angle (flat windows)
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("wedge");
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
        node_attr
            .create_property("wedge", "wedge angle", Angle::zero().into())
            .unwrap();

        let mut wedge = Self { node_attr };
        wedge.update_surfaces().unwrap();
        wedge
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

        wedge.update_surfaces()?;
        wedge.node_attr.set_property("wedge", wedge_angle.into())?;
        Ok(wedge)
    }
}

impl OpticNode for Wedge {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);

        let front_geosurface = GeoSurfaceRef(Rc::new(RefCell::new(Plane::new(node_iso.clone()))));

        self.update_surface(
            &"input_1".to_string(),
            front_geosurface,
            Isometry::identity(),
            &PortType::Input,
        )?;

        let Ok(Proptype::Length(center_thickness)) =
            self.node_attr.get_property("center thickness")
        else {
            return Err(OpossumError::Analysis(
                "cannot read center thickness".into(),
            ));
        };

        let angle = if let Ok(Proptype::Angle(wedge)) = self.node_attr.get_property("wedge") {
            *wedge
        } else {
            return Err(OpossumError::Analysis("cannot read wedge angle".into()));
        };

        let thickness_iso = Isometry::new_along_z(*center_thickness)?;
        let wedge_iso = Isometry::new(
            Point3::origin(),
            Point3::new(angle, Angle::zero(), Angle::zero()),
        )?;
        let anchor_point_iso = thickness_iso.append(&wedge_iso);
        let rear_geosurface = GeoSurfaceRef(Rc::new(RefCell::new(Plane::new(
            node_iso.append(&anchor_point_iso),
        ))));

        self.update_surface(
            &"output_1".to_string(),
            rear_geosurface,
            anchor_point_iso,
            &PortType::Output,
        )?;
        Ok(())
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
impl LIDT for Wedge {}

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
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn set_aperture() {
        test_set_aperture::<Wedge>("input_1", "output_1");
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
        input.insert("output_1".into(), input_light.clone());
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
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<Wedge>("input_1");
    }
    #[test]
    fn analyze_geometric_ok() {
        let mut node = Wedge::default();
        node.set_isometry(
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap(),
        )
        .unwrap();
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
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
