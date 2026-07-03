use std::sync::{Arc, Mutex};

#[cfg(test)]
use crate::refractive_index::RefractiveIndex;
use crate::{
    analyzers::energy::AnalysisEnergy,
    core_optics::{NodeAttr, OpticNode, OpticNodeExt, PortType},
    degree,
    error::{OpmResult, OpossumError},
    geometry::{Plane, geo_surface::GeoSurfaceRef},
    millimeter,
    nodes::NodeRegistration,
    properties::{Proptype, validator::Validator},
    refractive_index::{RefrIndexConst, RefractiveIndexType},
    utils::geom_transformation::Isometry,
};
use nalgebra::Point3;
use num::Zero;
use opm_macros_lib::OpmNode;
use uom::si::f64::{Angle, Length};

mod analysis_ghostfocus;
mod analysis_raytrace;

inventory::submit! {
    NodeRegistration::new::<Wedge>("wedge", "wedged substrate (prism)")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("aquamarine")]
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
            .create_property_with_validator(
                "center thickness",
                "thickness of the lens in the center",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsFinite, Validator::NumericIsPositive],
                },
                // and_validator(vec![numeric_is_positive(), numeric_is_finite()]),
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the lens material",
                RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()).into(),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                "wedge",
                "wedge angle",
                Validator::AndValidator {
                    validators: vec![
                        Validator::AngleInRange {
                            min: degree!(-90.),
                            max: degree!(90.),
                            inclusive: true,
                        },
                        Validator::NumericIsFinite,
                    ],
                },
                // and_validator(vec![
                //     angle_in_range(degree!(-90.0), degree!(90.0), true),
                //     numeric_is_finite(),
                // ]),
                Angle::zero().into(),
            )
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
    ///   - the center thickness is negative or not finite
    ///   - the wedge angle is outside ]-90°; 90°[ or not finite
    pub fn new(
        name: &str,
        center_thickness: Length,
        wedge_angle: Angle,
        refractive_index: impl Into<RefractiveIndexType>,
    ) -> OpmResult<Self> {
        let mut wedge = Self::default();
        wedge.node_attr.set_name(name);
        wedge
            .node_attr
            .set_property("center thickness", center_thickness.into())?;

        wedge
            .node_attr
            .set_property("refractive index", refractive_index.into().into())?;
        wedge.node_attr.set_property("wedge", wedge_angle.into())?;
        wedge.update_surfaces()?;
        Ok(wedge)
    }
}

impl OpticNode for Wedge {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);

        let front_geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));

        self.update_surface(
            "input_1",
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
        let rear_geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(
            node_iso.append(&anchor_point_iso),
        ))));

        self.update_surface(
            "output_1",
            rear_geosurface,
            anchor_point_iso,
            &PortType::Output,
        )?;
        Ok(())
    }
}
impl AnalysisEnergy for Wedge {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        core_optics::{NodeAttrExt, PortType},
        degree, joule,
        light::{LightData, LightResult, Ray, Rays, spectrum_helper::create_he_ne_spec},
        nanometer,
        nodes::test_helper::test_helper::*,
        properties::Proptype,
    };
    use nalgebra::Vector3;

    #[test]
    fn default() -> OpmResult<()> {
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
            if let RefractiveIndexType::Const(val) = &p {
                let idx = val.get_refractive_index(nanometer!(1000.0))?;
                assert_eq!(idx, 1.5);
            } else {
                assert!(false, "could not read refractive index constant.");
            }
        } else {
            assert!(false, "could not read refractive index.");
        }
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        assert!(
            Wedge::new(
                "test",
                millimeter!(-0.1),
                degree!(0.0),
                &RefrIndexConst::new(1.5)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(f64::NEG_INFINITY),
                degree!(0.0),
                &RefrIndexConst::new(1.5)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(f64::INFINITY),
                degree!(0.0),
                &RefrIndexConst::new(1.5)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(f64::NAN),
                degree!(0.0),
                &RefrIndexConst::new(1.5)?
            )
            .is_err()
        );

        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(f64::NEG_INFINITY),
                &RefrIndexConst::new(1.0)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(f64::INFINITY),
                &RefrIndexConst::new(1.0)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(f64::NAN),
                &RefrIndexConst::new(1.0)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(90.01),
                &RefrIndexConst::new(1.0)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(-90.01),
                &RefrIndexConst::new(1.0)?
            )
            .is_err()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(89.99),
                &RefrIndexConst::new(1.0)?
            )
            .is_ok()
        );
        assert!(
            Wedge::new(
                "test",
                millimeter!(0.0),
                degree!(-89.99),
                &RefrIndexConst::new(1.0)?
            )
            .is_ok()
        );
        let n = Wedge::new(
            "test",
            millimeter!(0.0),
            degree!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;
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
            if let RefractiveIndexType::Const(val) = &p {
                let idx = val.get_refractive_index(nanometer!(1000.0))?;
                assert_eq!(idx, 1.0);
            } else {
                assert!(false, "could not read refractive index constant.");
            }
        } else {
            assert!(false, "could not read refractive index.");
        }
        Ok(())
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
    fn inverted() -> OpmResult<()> {
        test_inverted::<Wedge>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<Wedge>()
    }
    #[test]
    fn analyze_wrong_port() -> OpmResult<()> {
        let mut node = Wedge::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_energy_ok() -> OpmResult<()> {
        let mut node = Wedge::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(*output, input_light);
        Ok(())
    }
    #[test]
    fn analyze_geometric_wrong_data_type() -> OpmResult<()> {
        test_analyze_wrong_data_type::<Wedge>("input_1")
    }
    #[test]
    fn analyze_geometric_ok() -> OpmResult<()> {
        let mut node = Wedge::default();
        node.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 10.0),
            degree!(0.0, 0.0, 0.0),
        )?)?;
        let mut input = LightResult::default();
        let rays = Rays::from(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?);
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            assert_eq!(rays.nr_of_rays(true), 1);
            let ray = rays.iter().next().unwrap();
            assert_eq!(ray.position(), millimeter!(0.0, 0.0, 20.0));
            let dir = Vector3::new(0.0_f64, 0.0, 1.0);
            assert_eq!(ray.direction(), dir);
        } else {
            assert!(false, "could not get LightData");
        }
        Ok(())
    }
}
