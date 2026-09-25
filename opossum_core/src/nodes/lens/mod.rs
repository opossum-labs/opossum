#![warn(missing_docs)]
//! Lens with spherical or flat surfaces

use crate::{
    analyzers::energy::AnalysisEnergy,
    apertures::ApertureShape,
    core_optics::{NodeAttr, OpticNode, OpticNodeExt, PortType, Volumetric},
    error::{OpmResult, OpossumError},
    geometry::{Plane, Sphere, body::CLEAR_APERTURE, geo_surface::GeoSurfaceRef},
    material::{MATERIAL, Material},
    meter, millimeter,
    nodes::{NodeRegistration, create_volume_properties},
    properties::{Proptype, validator::Validator},
    radian,
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
};
use log::warn;
use opm_macros_lib::OpmNode;
use std::sync::{Arc, Mutex};
use uom::si::f64::Length;

mod analysis_ghostfocus;
mod analysis_raytrace;

inventory::submit! {
    NodeRegistration::new::<Lens>("lens", "spherical lens")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("aqua")]
/// A real lens with spherical (or flat) surfaces.
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
///   - `material`
///   - `clear aperture`
pub struct Lens {
    node_attr: NodeAttr,
}

impl Default for Lens {
    /// Create a lens with a center thickness of 10.0 mm. front & back radii of curvature of 500.0 mm and a refractive index of 1.5.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("lens");
        node_attr
            .create_property_with_validator(
                "front curvature",
                "radius of curvature of front surface",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsNotZero, Validator::NumericIsNotNaN],
                },
                Proptype::Curvature(millimeter!(500.0)),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                "rear curvature",
                "radius of curvature of rear surface",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsNotZero, Validator::NumericIsNotNaN],
                },
                Proptype::Curvature(millimeter!(-500.0)),
            )
            .unwrap();
        node_attr
            .create_property_with_validator(
                "center thickness",
                "thickness of the lens in the center",
                Validator::AndValidator {
                    validators: vec![Validator::NumericIsFinite, Validator::NumericIsPositive],
                },
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                MATERIAL,
                "material of the lens",
                Material::new_draft(
                    "lens material",
                    None,
                    None,
                    RefrIndexConst::new(1.5).unwrap().into(),
                )
                .into(),
            )
            .unwrap();
        create_volume_properties(&mut node_attr).unwrap();
        let mut lens = Self { node_attr };
        lens.update_surfaces().unwrap();
        lens
    }
}
impl Lens {
    /// Creates a new [`Lens`].
    ///
    /// This function creates a lens with spherical front and back surfaces, a given center thickness and refractive index.
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
        material: impl Into<Material>,
    ) -> OpmResult<Self> {
        let mut lens = Self::default();
        lens.node_attr.set_name(name);
        lens.node_attr
            .set_property("front curvature", Proptype::Curvature(front_curvature))?;
        lens.node_attr
            .set_property("rear curvature", Proptype::Curvature(rear_curvature))?;
        lens.node_attr
            .set_property("center thickness", center_thickness.into())?;
        lens.node_attr
            .set_property(MATERIAL, material.into().into())?;
        lens.update_surfaces()?;
        Ok(lens)
    }

    // /// create a default aperture: defined by
    // ///  - intersection of two spheres
    // ///  - intersection of sphere and plane
    // ///  - the minimum radius of the spheres if there is no intersection
    // fn get_minimum_logical_aperture_radius(
    //     front_curvature: Length,
    //     rear_curvature: Length,
    //     center_thickness: Length,
    // ) -> Option<Length> {
    //     // case 1: bi-convex
    //     if front_curvature.is_sign_positive()
    //         && front_curvature.is_finite()
    //         && rear_curvature.is_sign_negative()
    //         && rear_curvature.is_finite()
    //     {
    //         //get intersecting radius by calculating the area of a triangle that is defined the two radii and the distance between the sphere centers
    //         let sphere_dist = rear_curvature.abs() + front_curvature.abs() - center_thickness;
    //         let semiperimeter = 0.5 * (sphere_dist + rear_curvature.abs() + front_curvature.abs());
    //         //herons formula
    //         let triangle_area = (semiperimeter
    //             * (semiperimeter - sphere_dist)
    //             * (semiperimeter - rear_curvature.abs())
    //             * (semiperimeter - front_curvature.abs()))
    //         .sqrt();
    //         //setting equal two are defined by base height x base length / 2 and rearrange
    //         Some(triangle_area / sphere_dist * 2.)
    //     }
    //     // case 2a: plano-convex with back plane
    //     else if front_curvature.is_sign_positive()
    //         && front_curvature.is_finite()
    //         && rear_curvature.is_infinite()
    //     {
    //         Some(
    //             (front_curvature * front_curvature
    //                 - (front_curvature.abs() - center_thickness)
    //                     * (front_curvature.abs() - center_thickness))
    //                 .sqrt(),
    //         )
    //     }
    //     // case 2b: plano-convex with front plane
    //     else if rear_curvature.is_sign_negative()
    //         && rear_curvature.is_finite()
    //         && front_curvature.is_infinite()
    //     {
    //         Some(
    //             (rear_curvature * rear_curvature
    //                 - (rear_curvature.abs() - center_thickness)
    //                     * (rear_curvature.abs() - center_thickness))
    //                 .sqrt(),
    //         )
    //     }
    //     // case 3: positive meniscus lens
    //     else if front_curvature.is_sign_positive()
    //         && rear_curvature.is_sign_positive()
    //         && front_curvature >= rear_curvature
    //         && front_curvature.is_finite()
    //         || front_curvature.is_sign_negative()
    //             && rear_curvature.is_sign_negative()
    //             && front_curvature <= rear_curvature
    //             && rear_curvature.is_finite()
    //     {
    //         let g = front_curvature.abs() - (rear_curvature.abs() - center_thickness);
    //         let semiperimeter = 0.5 * (g + rear_curvature.abs() + front_curvature.abs());
    //         let triangle_area = (semiperimeter
    //             * (semiperimeter - g)
    //             * (semiperimeter - rear_curvature.abs())
    //             * (semiperimeter - front_curvature.abs()))
    //         .sqrt();
    //         Some(
    //             2. * triangle_area
    //                 / (front_curvature.abs() + center_thickness - rear_curvature.abs()),
    //         )
    //     }
    //     //case 4: flat flat. no defined aperture. set to infinity
    //     else if front_curvature.is_infinite() && rear_curvature.is_infinite() {
    //         None
    //     }
    //     // case 5: negative meniscus lens or bi-concave or plano-concave
    //     // get the minimum of both radii
    //     else if front_curvature.abs() < rear_curvature.abs() {
    //         Some(front_curvature.abs())
    //     } else {
    //         Some(rear_curvature.abs())
    //     }
    // }
}

impl Volumetric for Lens {}
impl OpticNode for Lens {
    fn as_volume(&self) -> Option<&dyn Volumetric> {
        Some(self)
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);
        let Ok(Proptype::Curvature(front_curvature)) =
            self.node_attr.get_property("front curvature")
        else {
            return Err(OpossumError::Analysis("cannot read front curvature".into()));
        };
        check_curvature_fits_circular_aperture(*front_curvature, &self.node_attr, "front")?;
        let (front_geosurface, anchor_point_iso_front) = if front_curvature.is_infinite() {
            (
                GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso)))),
                Isometry::identity(),
            )
        } else {
            let anchor_point_iso_front =
                Isometry::new(meter!(0., 0., front_curvature.value), radian!(0., 0., 0.))?;
            (
                GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(
                    *front_curvature,
                    node_iso.append(&anchor_point_iso_front),
                )?))),
                anchor_point_iso_front,
            )
        };
        self.update_surface(
            "input_1",
            front_geosurface,
            anchor_point_iso_front,
            &PortType::Input,
        )?;
        let Ok(Proptype::Curvature(rear_curvature)) = self.node_attr.get_property("rear curvature")
        else {
            return Err(OpossumError::Analysis("cannot read rear curvature".into()));
        };
        check_curvature_fits_circular_aperture(*rear_curvature, &self.node_attr, "rear")?;
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
                GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(
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
                GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(
                    *rear_curvature,
                    node_iso.append(&anchor_point_iso_rear),
                )?))),
                anchor_point_iso_rear,
            )
        };
        self.update_surface(
            "output_1",
            rear_geosurface,
            anchor_point_iso_rear,
            &PortType::Output,
        )
    }
}

/// Reject a curvature radius that cannot span its own clear aperture.
///
/// A spherical surface curves back on itself at its own radius of curvature - beyond that distance
/// from the axis it no longer lies above the transversal plane at all (see
/// [`curved_local_z`](crate::geometry::geo_surface)'s doc comment). A curvature radius smaller in
/// magnitude than the clear aperture's radius therefore describes a surface that cannot reach the
/// rim of its own aperture: not a shape that merely fails to mesh later, but one that was never
/// geometrically possible. Equality is allowed - a curvature radius exactly equal to the aperture
/// radius is a hemisphere, the tightest valid case, and `curved_local_z`'s own tolerance is what
/// lets that exact case actually be meshed.
///
/// Checked here, as the first thing [`Lens::update_surfaces`] does with each curvature, rather than
/// expressed as a [`Validator`]: `Validator::validate` sees only the one property being set, never a
/// sibling one, so a curvature-versus-aperture comparison cannot be written as one. Only a circular
/// clear aperture is checked, which is the case this exists for - see `known_issue_update_surfaces.md`
/// and C1 of the plan this belongs to, both about exactly this scenario - and the only
/// [`ApertureShape`] with a single radius to compare against; a non-circular clear aperture is left
/// to fail at meshing time as before.
///
/// # Arguments
///
/// * `curvature` - the front or rear curvature radius, as read from the node's properties.
/// * `node_attr` - the node's attributes, to read the clear aperture from.
/// * `surface_name` - `"front"` or `"rear"`, to name the offending surface in the error.
///
/// # Errors
///
/// Returns an error if the clear aperture is circular and `curvature`'s magnitude is smaller than
/// its radius.
fn check_curvature_fits_circular_aperture(
    curvature: Length,
    node_attr: &NodeAttr,
    surface_name: &str,
) -> OpmResult<()> {
    if curvature.is_infinite() {
        // Flat - every clear aperture fits under a plane.
        return Ok(());
    }
    let Ok(Proptype::Aperture(ApertureShape::BinaryCircle(circle))) =
        node_attr.get_property(CLEAR_APERTURE)
    else {
        return Ok(());
    };
    let aperture_radius = circle.radius();
    if curvature.abs() < aperture_radius {
        return Err(OpossumError::Properties(format!(
            "the {surface_name} curvature radius ({curvature:?}) is smaller than the clear \
             aperture radius ({aperture_radius:?}): the surface cannot reach the rim of its own \
             aperture"
        )));
    }
    Ok(())
}

impl AnalysisEnergy for Lens {}
// impl SDF for Lens
// {
//     fn sdf_eval_point(&self, p: &nalgebra::Point3<f64>, p_out: &mut nalgebra::Point3<f64>) -> f64 {
//         self.isometry.inverse_transform_point_mut_f64(&p, p_out);
//         // (p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - self.radius.value
//         (p_out.x.mul_add(p_out.x, p_out.y.mul_add(p_out.y, p_out.z*p_out.z)) ).sqrt() - self.radius.value
//     }
// }
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::{
            RayTraceConfig,
            energy::{AnalysisEnergy, EnergyConfig},
            raytrace::AnalysisRayTrace,
        },
        apertures::{ApertureShape, CircleShape},
        core_optics::{NodeAttrExt, node_attr::NodePositioning},
        distributions::position::Hexapolar,
        joule,
        light::{LightData, LightResult, Rays},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        properties::{Proptype, proptype::AssetRef},
    };
    use approx::assert_relative_eq;
    use core::f64;
    use nalgebra::Vector3;
    use num_traits::Zero;

    #[test]
    fn default() -> OpmResult<()> {
        let node = Lens::default();
        assert_eq!(node.name(), "lens");
        assert_eq!(node.node_type(), "lens");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "aqua");
        let Ok(Proptype::Curvature(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(500.0));
        let Ok(Proptype::Curvature(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(-500.0));
        let Ok(Proptype::Length(ct)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*ct, millimeter!(10.0));
        let Ok(Proptype::Material(AssetRef::Inline(material))) =
            node.node_attr.get_property(MATERIAL)
        else {
            panic!()
        };
        assert_eq!((*material).get_refractive_index(Length::zero())?, 1.5);
        Ok(())
    }
    #[test]
    fn set_front_curvature() {
        let mut lens = Lens::default();
        assert!(
            lens.node_attr
                .set_property("front curvature", Proptype::Curvature(Length::zero()))
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "front curvature",
                    Proptype::Curvature(millimeter!(f64::NAN))
                )
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "front curvature",
                    Proptype::Curvature(millimeter!(f64::INFINITY))
                )
                .is_ok()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "front curvature",
                    Proptype::Curvature(millimeter!(f64::NEG_INFINITY))
                )
                .is_ok()
        );
    }
    #[test]
    fn set_rear_curvature() {
        let mut lens = Lens::default();
        assert!(
            lens.node_attr
                .set_property("rear curvature", Proptype::Curvature(Length::zero()))
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property("rear curvature", Proptype::Curvature(millimeter!(f64::NAN)))
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "rear curvature",
                    Proptype::Curvature(millimeter!(f64::INFINITY))
                )
                .is_ok()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "rear curvature",
                    Proptype::Curvature(millimeter!(f64::NEG_INFINITY))
                )
                .is_ok()
        );
    }
    #[test]
    fn set_center_thickness() {
        let mut lens = Lens::default();
        assert!(
            lens.node_attr
                .set_property("center thickness", Proptype::Length(millimeter!(-0.1)))
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property("center thickness", Proptype::Length(millimeter!(0.0)))
                .is_ok()
        );
        assert!(
            lens.node_attr
                .set_property("center thickness", Proptype::Length(millimeter!(f64::NAN)))
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "center thickness",
                    Proptype::Length(millimeter!(f64::INFINITY))
                )
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property(
                    "center thickness",
                    Proptype::Length(millimeter!(f64::NEG_INFINITY))
                )
                .is_err()
        );
    }
    #[test]
    fn new() -> OpmResult<()> {
        let roc = millimeter!(100.0);
        let ct = millimeter!(11.0);
        let ref_index = RefrIndexConst::new(1.5)?;

        assert!(Lens::new("test", roc, roc, millimeter!(-0.1), &ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::NAN), &ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::INFINITY), &ref_index).is_err());

        assert!(Lens::new("test", roc, Length::zero(), ct, &ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::NAN), ct, &ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::INFINITY), ct, &ref_index).is_ok());
        assert!(Lens::new("test", roc, millimeter!(f64::NEG_INFINITY), ct, &ref_index).is_ok());

        assert!(Lens::new("test", Length::zero(), roc, ct, &ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::NAN), roc, ct, &ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::INFINITY), roc, ct, &ref_index).is_ok());
        assert!(Lens::new("test", millimeter!(f64::NEG_INFINITY), roc, ct, &ref_index).is_ok());
        let ref_index = RefrIndexConst::new(2.0)?;
        let node = Lens::new("test", roc, roc, ct, &ref_index)?;
        assert_eq!(node.name(), "test");
        let Ok(Proptype::Curvature(roc)) = node.node_attr.get_property("front curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Curvature(roc)) = node.node_attr.get_property("rear curvature") else {
            panic!()
        };
        assert_eq!(*roc, millimeter!(100.0));
        let Ok(Proptype::Length(ct)) = node.node_attr.get_property("center thickness") else {
            panic!()
        };
        assert_eq!(*ct, millimeter!(11.0));
        let Ok(Proptype::Material(AssetRef::Inline(material))) =
            node.node_attr.get_property(MATERIAL)
        else {
            panic!()
        };
        assert_eq!((*material).get_refractive_index(Length::zero())?, 2.0);
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<Lens>()
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<Lens>()
    }
    #[test]
    fn analyze_wrong_port() -> OpmResult<()> {
        let mut node = Lens::default();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_geometric_wrong_data_type() -> OpmResult<()> {
        test_analyze_wrong_data_type::<Lens>("input_1")
    }
    #[test]
    fn analyze_flatflat() -> OpmResult<()> {
        let mut node = Lens::new(
            "test",
            millimeter!(f64::INFINITY),
            millimeter!(f64::NEG_INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(2.0)?,
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::new_along_z(
            millimeter!(10.0),
        )?))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3)?,
        )?;
        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())?;
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
                assert_eq!(ray.path_length(), millimeter!(30.0));
            }
        } else {
            assert!(false);
        }
        Ok(())
    }
    #[test]
    fn analyze_biconvex() -> OpmResult<()> {
        // biconvex lens with index of 1.0 (="neutral" lens)
        let mut node = Lens::new(
            "test",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3)?,
        )?;
        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".into(), LightData::Geometric(rays));
        let output =
            AnalysisRayTrace::analyze(&mut node, incoming_data, &RayTraceConfig::default())?;
        if let Some(LightData::Geometric(rays)) = output.get("output_1") {
            for ray in rays {
                assert_eq!(ray.direction(), Vector3::z());
            }
        } else {
            assert!(false);
        }
        Ok(())
    }
    /// Reference values for the entry surface → volume → exit surface propagation.
    ///
    /// This pins the current behaviour down completely so that refactoring the two-surface
    /// sequence in `analysis_raytrace.rs` can be verified to be behaviour-neutral. The values are
    /// recorded, not derived — physical correctness is covered by the other tests in this module.
    #[test]
    fn volume_propagation_regression() -> OpmResult<()> {
        let mut node = Lens::new(
            "regression",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            RefrIndexConst::new(1.5)?,
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        test_volume_propagation_regression(
            &mut node,
            &[
                [0.0, 0.0, 10.0, 0.0, 0.0, 1.0, 1.0, 15.0],
                [
                    4.837_210_642_342,
                    0.0,
                    9.882_938_448_974,
                    -0.049_284_003_372,
                    0.0,
                    0.998_784_805_157,
                    1.0,
                    14.763_905_268_675,
                ],
                [
                    0.331_985_150_282,
                    -3.203_693_130_790,
                    9.948_117_221_805,
                    0.047_993_663_920,
                    0.135_562_167_588,
                    0.989_605_733_079,
                    1.0,
                    14.938_120_918_052,
                ],
            ],
        )
    }
    #[test]
    fn volume_body() -> OpmResult<()> {
        test_volume_body::<Lens>()
    }
    #[test]
    fn clear_aperture() -> OpmResult<()> {
        test_clear_aperture::<Lens>()
    }
    #[test]
    fn clear_aperture_absent_in_file() -> OpmResult<()> {
        test_clear_aperture_absent_in_file::<Lens>()
    }
    /// The body of a biconvex lens is thinner towards its rim by the sag of both surfaces.
    #[test]
    fn volume_body_thins_out_towards_the_rim() -> OpmResult<()> {
        let curvature = millimeter!(100.0);
        let center_thickness = millimeter!(10.0);
        let ray_position = millimeter!(5.0, 0.0, 0.0);
        let ray_height = ray_position.x;
        let mut node = Lens::new(
            "rim",
            curvature,
            -curvature,
            center_thickness,
            RefrIndexConst::new(1.5)?,
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let path_length = path_length_through(&node, ray_position)?;
        // Both surfaces recede by the same sag, so the volume is two sags thinner at that height.
        let sag = curvature - (curvature * curvature - ray_height * ray_height).sqrt();
        assert_relative_eq!(
            path_length.value,
            (center_thickness - 2.0 * sag).value,
            epsilon = 1e-12
        );
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_bi_convex() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.031224989991992);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.031224989991992);
        }
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_plano_convex() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(f64::INFINITY),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.04358898943540674);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.04358898943540674);
        }

        let node = Lens::new(
            "test",
            millimeter!(100.),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.04358898943540674);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.04358898943540674);
        }
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_bi_concave() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(-100.0),
            millimeter!(100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }

        let node = Lens::new(
            "test",
            millimeter!(-200.0),
            millimeter!(100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }

        let node = Lens::new(
            "test",
            millimeter!(-100.0),
            millimeter!(200.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_plano_concave() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(f64::INFINITY),
            millimeter!(100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }

        let node = Lens::new(
            "test",
            millimeter!(-100.0),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_pos_meniscus() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(-105.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09637888196533964);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09637888196533964);
        }

        let node = Lens::new(
            "test",
            millimeter!(105.0),
            millimeter!(100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09637888196533964);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09637888196533964);
        }

        let node = Lens::new(
            "test",
            millimeter!(-100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09987492177719105);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09987492177719105);
        }

        let node = Lens::new(
            "test",
            millimeter!(100.0),
            millimeter!(100.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09987492177719105);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 0.09987492177719105);
        }
        Ok(())
    }
    #[test]
    fn get_minimum_logical_aperture_radius_neg_meniscus() -> OpmResult<()> {
        let node = Lens::new(
            "test",
            millimeter!(-100.0),
            millimeter!(-105.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }

        let node = Lens::new(
            "test",
            millimeter!(100.0),
            millimeter!(105.0),
            millimeter!(10.0),
            &RefrIndexConst::new(1.0)?,
        )?;

        assert!(node.ports().aperture(&PortType::Input, "input_1").is_some());
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Input, "input_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        assert!(
            node.ports()
                .aperture(&PortType::Output, "output_1")
                .is_some()
        );
        if let Some(ApertureShape::BinaryCircle(c)) = node
            .ports()
            .aperture(&PortType::Output, "output_1")
            .map(|a| a.shape())
        {
            assert_relative_eq!(c.radius().value, 100e-3);
        }
        Ok(())
    }
    #[test]
    fn analyze_lens_matching_ambient_medium() -> OpmResult<()> {
        let n_match = 1.5;
        let mat = Material::new_draft(
            "matching_material",
            None,
            None,
            RefrIndexConst::new(n_match)?.into(),
        );

        let mut node = Lens::new(
            "test_lens",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            mat.clone(),
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;

        let mut config = RayTraceConfig::default();
        config.set_ambient_material(mat);

        // Create rays propagating in the ambient medium
        let mut rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3)?,
        )?;
        rays.set_refractive_index(&config.ambient_material().refractive_index_type())?;

        let mut incoming_data = LightResult::default();
        incoming_data.insert("input_1".into(), LightData::Geometric(rays));

        let output = AnalysisRayTrace::analyze(&mut node, incoming_data, &config)?;
        if let Some(LightData::Geometric(output_rays)) = output.get("output_1") {
            for ray in output_rays {
                // Direction must remain undisturbed along the optical axis
                approx::assert_relative_eq!(ray.direction(), Vector3::z(), epsilon = 1e-12);
                approx::assert_relative_eq!(ray.refractive_index(), n_match, epsilon = 1e-12);
            }
        } else {
            panic!("Expected LightData::Geometric at output_1");
        }
        Ok(())
    }

    /// A lens whose curvature radius exactly equals its clear aperture radius is a hemisphere - the
    /// tightest a spherical surface can be without folding back past its own equator. Geometrically
    /// this is exactly valid (the sag at the rim is the radius itself, the surface just reaches flat
    /// tangent to the aperture plane there), so this pins down whether it can actually be meshed, or
    /// whether the floating-point boundary in `curved_local_z` rejects it as "not reaching as far
    /// out as" the aperture. See `known_issue_update_surfaces.md`'s sibling investigation notes.
    #[test]
    fn a_hemisphere_lens_can_be_meshed() -> OpmResult<()> {
        let hemisphere_radius = millimeter!(25.0);
        let aperture: Proptype = ApertureShape::from(CircleShape::new(hemisphere_radius)?).into();

        // Plano-convex: one flat face, one exact hemisphere - the centre thickness has to reach
        // that one sag (25 mm) or the hemisphere's rim would poke out past the flat face.
        let mut plano_convex = Lens::new(
            "plano-convex hemisphere",
            millimeter!(f64::INFINITY),
            -hemisphere_radius,
            hemisphere_radius,
            RefrIndexConst::new(1.5)?,
        )?;
        plano_convex
            .node_attr
            .set_property("clear aperture", aperture.clone())?;
        plano_convex
            .as_volume()
            .expect("a lens is volumetric")
            .volume_body()?
            .triangulate(64)?;

        // Bi-convex: both faces an exact hemisphere, so the centre thickness must reach both sags
        // (25 mm each) or the two surfaces would cross inside the lens.
        let mut bi_convex = Lens::new(
            "bi-convex hemisphere",
            hemisphere_radius,
            -hemisphere_radius,
            millimeter!(50.0),
            RefrIndexConst::new(1.5)?,
        )?;
        bi_convex
            .node_attr
            .set_property("clear aperture", aperture)?;
        bi_convex
            .as_volume()
            .expect("a lens is volumetric")
            .volume_body()?
            .triangulate(64)?;

        Ok(())
    }
}
