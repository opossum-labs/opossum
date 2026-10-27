#![warn(missing_docs)]
//! Lens with spherical or flat surfaces

use crate::{
    analyzers::energy::AnalysisEnergy,
    core_optics::{Appearance, NodeAttr, OpticNode, OpticNodeExt, SurfacePorts, Volumetric},
    error::{OpmResult, OpossumError},
    geometry::{
        Geometry,
        body::{OPTIC_SHAPE, default_optic_shape},
    },
    material::{MATERIAL, Material},
    millimeter,
    nodes::NodeRegistration,
    properties::{Proptype, validator::Validator},
    refractive_index::RefrIndexConst,
};
use log::warn;
use opm_macros_lib::OpmNode;
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
///   - `optic shape`
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
        node_attr
            .create_property_with_validator(
                OPTIC_SHAPE,
                "transversal extent of the component",
                Validator::OpticShapeIsBounded,
                default_optic_shape().into(),
            )
            .unwrap();
        let mut lens = Self { node_attr };
        lens.create_default_surface_properties().unwrap();
        lens.update_surfaces().unwrap();
        lens
    }
}
impl Lens {
    /// Creates a new [`Lens`].
    ///
    /// This function creates a lens with spherical front and back surfaces, a given center
    /// thickness and material, bounded by the default optic shape ([`default_optic_shape`]). The
    /// radii of curvature must not be zero. A radius of curvature of +/- infinity corresponds to a
    /// flat surface. Another optic shape is set afterwards via the [`OPTIC_SHAPE`] property.
    ///
    /// # Arguments
    ///
    /// * `name` - the name of the lens.
    /// * `front_curvature` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_curvature` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `material` - the material of the lens.
    ///
    /// # Errors
    ///
    /// This function returns an error if the given parameters are not correct, including a face
    /// curved too tightly to reach the edge of the default optic shape. A lens curved that tightly
    /// is built from [`Lens::default`]: set its optic shape first, then its curvatures.
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
}

impl Volumetric for Lens {}
impl OpticNode for Lens {
    fn create_geometry(&self) -> OpmResult<Option<Geometry>> {
        let (
            Ok(Proptype::Curvature(front_curvature)),
            Ok(Proptype::Curvature(rear_curvature)),
            Ok(Proptype::Length(center_thickness)),
        ) = (
            self.node_attr.get_property("front curvature"),
            self.node_attr.get_property("rear curvature"),
            self.node_attr.get_property("center thickness"),
        )
        else {
            return Err(OpossumError::Analysis(
                "cannot read the curvatures and the center thickness of the lens".into(),
            ));
        };
        let cross_section = self.cross_section()?.ok_or_else(|| {
            OpossumError::Analysis(format!(
                "the {OPTIC_SHAPE} of node '{}' is open, so its medium is unbounded",
                self.node_attr.name()
            ))
        })?;
        Ok(Some(Geometry::singlet(
            *front_curvature,
            *rear_curvature,
            *center_thickness,
            cross_section,
            self.material()?,
        )?))
    }
    fn as_volume(&self) -> Option<&dyn Volumetric> {
        Some(self)
    }
    fn appearance(&self) -> Option<Appearance> {
        Some(Appearance::glass())
    }
    fn surface_ports(&self) -> SurfacePorts {
        SurfacePorts::THROUGH_SOLID
    }
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
        apertures::{CircleShape, OpticShape},
        core_optics::{NodeAttrExt, node_attr::NodePositioning},
        distributions::position::Hexapolar,
        joule,
        light::{LightData, LightResult, Rays},
        millimeter, nanometer,
        nodes::test_helper::helper::*,
        properties::{Proptype, proptype::AssetRef},
        utils::geom_transformation::Isometry,
    };
    use approx::assert_relative_eq;
    use nalgebra::Vector3;
    use num_traits::Zero;

    #[test]
    fn default() -> OpmResult<()> {
        let node = Lens::default();
        assert_eq!(node.name(), "lens");
        assert_eq!(node.node_type(), "lens");
        assert!(!node.inverted());
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
                .is_err()
        );
        assert!(
            lens.node_attr
                .set_property("center thickness", Proptype::Length(millimeter!(0.0001)))
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

        assert!(Lens::new("test", roc, roc, millimeter!(-0.1), ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::NAN), ref_index).is_err());
        assert!(Lens::new("test", roc, roc, millimeter!(f64::INFINITY), ref_index).is_err());

        assert!(Lens::new("test", roc, Length::zero(), ct, ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::NAN), ct, ref_index).is_err());
        assert!(Lens::new("test", roc, millimeter!(f64::INFINITY), ct, ref_index).is_ok());
        assert!(Lens::new("test", roc, millimeter!(f64::NEG_INFINITY), ct, ref_index).is_ok());

        assert!(Lens::new("test", Length::zero(), roc, ct, ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::NAN), roc, ct, ref_index).is_err());
        assert!(Lens::new("test", millimeter!(f64::INFINITY), roc, ct, ref_index).is_ok());
        assert!(Lens::new("test", millimeter!(f64::NEG_INFINITY), roc, ct, ref_index).is_ok());
        let ref_index = RefrIndexConst::new(2.0)?;
        let node = Lens::new("test", roc, roc, ct, ref_index)?;
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
    fn rays_beyond_optic_shape_are_lost() -> OpmResult<()> {
        test_rays_beyond_optic_shape_are_lost::<Lens>()
    }
    /// A lens whose curvature is tighter than its optic shape describes no shape: a sphere of
    /// 10 mm radius ends before the default rim of 12.5 mm. The lens refuses it.
    #[test]
    fn a_curvature_tighter_than_the_optic_shape_is_rejected() -> OpmResult<()> {
        let flat = millimeter!(f64::INFINITY);
        let index = RefrIndexConst::new(1.5)?;
        assert!(
            Lens::new(
                "lens",
                millimeter!(10.0),
                flat,
                millimeter!(5.0),
                index.clone()
            )
            .is_err()
        );
        assert!(Lens::new("lens", flat, millimeter!(-10.0), millimeter!(5.0), index).is_err());
        Ok(())
    }
    /// A lens smaller than the default optic shape is built with its own, set before its
    /// curvatures: a concentric meniscus of 10 mm and 9 mm radius fits an optic shape of 9 mm,
    /// its rear face a full hemisphere.
    #[test]
    fn a_small_lens_is_built_with_its_own_optic_shape() -> OpmResult<()> {
        let optic_shape: OpticShape = CircleShape::new(millimeter!(9.0))?.into();
        let mut lens = Lens::default();
        lens.set_property(OPTIC_SHAPE, optic_shape.clone().into())?;
        lens.set_property("front curvature", Proptype::Curvature(millimeter!(10.0)))?;
        lens.set_property("rear curvature", Proptype::Curvature(millimeter!(9.0)))?;
        lens.set_property("center thickness", millimeter!(1.0).into())?;
        lens.update_surfaces()?;
        assert_eq!(
            lens.node_attr.get_property(OPTIC_SHAPE)?,
            &Proptype::OpticShape(optic_shape)
        );
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
            RefrIndexConst::new(2.0)?,
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
            panic!();
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
            RefrIndexConst::new(1.0)?,
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
            panic!();
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
    fn optic_shape() -> OpmResult<()> {
        test_optic_shape::<Lens>(millimeter!(12.5))
    }
    #[test]
    fn optic_shape_absent_in_file() -> OpmResult<()> {
        test_optic_shape_absent_in_file::<Lens>()
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
        rays.set_refractive_index(config.ambient_material().refractive_index_type())?;

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
        let aperture: Proptype = OpticShape::from(CircleShape::new(hemisphere_radius)?).into();

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
            .set_property(OPTIC_SHAPE, aperture.clone())?;
        plano_convex.geometry_mesh(64)?;

        // Bi-convex: both faces an exact hemisphere, so the centre thickness must reach both sags
        // (25 mm each) or the two surfaces would cross inside the lens.
        let mut bi_convex = Lens::new(
            "bi-convex hemisphere",
            hemisphere_radius,
            -hemisphere_radius,
            millimeter!(50.0),
            RefrIndexConst::new(1.5)?,
        )?;
        bi_convex.node_attr.set_property(OPTIC_SHAPE, aperture)?;
        bi_convex.geometry_mesh(64)?;

        Ok(())
    }
}
