//!for all the functions, structs or trait that may be used for geometrical transformations
#![warn(missing_docs)]
use crate::{
    degree,
    error::{OpmResult, OpossumError},
    meter,
    properties::Proptype,
    radian,
};
use approx::relative_eq;
#[cfg(feature = "bevy")]
use bevy::{
    math::{Quat, Vec3, Vec4},
    transform::components::Transform,
};
use nalgebra::{Isometry3, MatrixXx2, MatrixXx3, Point3, Vector3};
use num::Zero;
use serde::{Deserialize, Serialize};
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
    length::meter,
};

use super::EnumProxy;

/// Struct to store the isometric transofmeation matrix and its inverse
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Isometry {
    transform: Isometry3<f64>,
    inverse: Isometry3<f64>,
}
impl Isometry {
    /// Creates a new [`Isometry`] which stores the rotation and translation as a transform matrix and its inverse.
    /// Internally, translation is handled in meter, rotation in radians
    /// # Attributes
    /// - `translation`: vector of translation for each axis as [`Length`]
    /// - `axisangle`: vector of rotation for each axis as [`Angle`]
    ///
    /// # Errors
    /// his function return an error if the
    ///  - the translation coordinates are not finite
    ///  - the axis angles are not finite
    pub fn new(translation: Point3<Length>, axisangle: Point3<Angle>) -> OpmResult<Self> {
        if translation.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "translation coordinates must be finite".into(),
            ));
        }
        if axisangle.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other("axis angles must be finite".into()));
        }
        let trans_in_m = Vector3::from_vec(
            translation
                .iter()
                .map(Length::get::<meter>)
                .collect::<Vec<f64>>(),
        );
        let rot_in_radian = Vector3::from_vec(
            axisangle
                .iter()
                .map(Angle::get::<radian>)
                .collect::<Vec<f64>>(),
        );
        Ok(Self::new_from_transform(Isometry3::new(
            trans_in_m,
            rot_in_radian,
        )))
    }
    /// Create a "identiy" Isometry, which represents a zero translation and rotation.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            transform: Isometry3::<f64>::identity(),
            inverse: Isometry3::<f64>::identity(),
        }
    }
    /// Create a new [`Isometry`] representing a translation along the z axis.
    ///
    /// This function is a convenience function and might be removed later.
    /// # Errors
    ///
    /// This function will return an error if the given z position is not finite.
    pub fn new_along_z(z_position: Length) -> OpmResult<Self> {
        Self::new(
            Point3::new(Length::zero(), Length::zero(), z_position),
            degree!(0.0, 0.0, 0.0),
        )
    }
    /// Creates a new [`Isometry`] which stores the rotation and translation as a transform matrix and its inverse.
    /// For the translation, it is assumed to transform a point of meters
    /// # Attributes
    /// - `view_point`: origin of the view. will translate the cartesian origin to this point
    /// - `view_direction`: view direction vector. The rotation will rotate the z vector onto this direction
    /// - `up_direction`: vertical direction of the view. Must not be collinear to the view!
    #[must_use]
    pub fn new_from_view(
        view_point: Point3<Length>,
        view_direction: Vector3<f64>,
        up_direction: Vector3<f64>,
    ) -> Self {
        //get translation vector
        let view_point_in_m = Point3::from_slice(
            &view_point
                .iter()
                .map(Length::get::<meter>)
                .collect::<Vec<f64>>(),
        );
        let target_point_in_m = view_point_in_m + view_direction;

        Self::new_from_transform(Isometry3::face_towards(
            &view_point_in_m,
            &target_point_in_m,
            &up_direction,
        ))
    }
    /// Creates a new isometry which stores the rotation and translation as a transform matrix and its inverse.
    /// For the translation, it is assumed to transform a point of meters
    /// # Attributes
    /// - `view_point`: origin of the view. will translate the cartesian origin to this point
    /// - `target_point`: target of the view.
    /// - `up_direction`: vertical direction of the view. Must not be collinear to the view!
    /// # Returns
    /// Returns a new [`Isometry`] struct
    #[must_use]
    pub fn new_from_view_on_target(
        view_point: Point3<Length>,
        target_point: Point3<Length>,
        up_direction: Vector3<f64>,
    ) -> Self {
        //get translation vector
        let view_point_in_m =
            Point3::from_slice(&view_point.iter().map(|x| x.value).collect::<Vec<f64>>());
        let target_point_in_m =
            Point3::from_slice(&target_point.iter().map(|x| x.value).collect::<Vec<f64>>());

        Self::new_from_transform(Isometry3::face_towards(
            &view_point_in_m,
            &target_point_in_m,
            &up_direction,
        ))
    }
    /// Add another [`Isometry`] to this [`Isometry`].
    ///
    /// This function "chains" two isometries (translation & rotation)
    #[must_use]
    pub fn append(&self, rhs: &Self) -> Self {
        let new_transform = self.transform * rhs.transform;
        let new_inverse = new_transform.inverse();
        Self {
            transform: new_transform,
            inverse: new_inverse,
        }
    }
    /// Creates a new isometry which stores the rotation and translation as a transform matrix and its inverse.
    /// The struct is created from an already exisiting tranformation isometry3
    #[must_use]
    pub fn new_from_transform(transform: Isometry3<f64>) -> Self {
        let inverse = transform.inverse();
        Self { transform, inverse }
    }
    /// Returns the translation vector of this [`Isometry`].
    #[must_use]
    pub fn translation_vec(&self) -> Vector3<Length> {
        let t = self.transform.translation * Point3::origin();
        Vector3::new(meter!(t.x), meter!(t.y), meter!(t.z))
    }
    /// Returns the translation of this [`Isometry`].
    #[must_use]
    pub fn translation(&self) -> Point3<Length> {
        let t = self.transform.translation;
        meter!(t.x, t.y, t.z)
    }
    /// Returns the rotation of this [`Isometry`].
    #[must_use]
    pub fn rotation(&self) -> Point3<Angle> {
        let rot = self.transform.rotation.euler_angles();
        radian!(rot.0, rot.1, rot.2)
    }
    /// Transforms a single point by the defined isometry
    /// # Attributes
    /// - `p`: Point3 with Length components
    /// # Returns
    /// Returns the transformed point3
    #[must_use]
    pub fn transform_point(&self, p: &Point3<Length>) -> Point3<Length> {
        let p_in_m = Point3::new(p.x.get::<meter>(), p.y.value, p.z.value);
        let p_iso_trans = self.transform.transform_point(&p_in_m);
        meter!(p_iso_trans.x, p_iso_trans.y, p_iso_trans.z)
    }

    /// Transforms a vector of points by the defined isometry
    /// # Attributes
    /// - `p_vec`: Vec of Point3 with Length components
    /// # Returns
    /// Returns the transformed point3s as Vec
    #[must_use]
    pub fn transform_points(&self, p_vec: &[Point3<Length>]) -> Vec<Point3<Length>> {
        p_vec
            .iter()
            .map(|p| self.transform_point(p))
            .collect::<Vec<Point3<Length>>>()
    }

    /// Inverse transforms a single point by the defined isometry
    /// # Attributes
    /// - `p`: Point3 with Length components
    /// # Returns
    /// Returns the inverse-transformed point3
    #[must_use]
    pub fn inverse_transform_point(&self, p: &Point3<Length>) -> Point3<Length> {
        let p_in_m = Point3::new(p.x.value, p.y.value, p.z.value);
        let p_iso_trans = self.inverse.transform_point(&p_in_m);
        meter!(p_iso_trans.x, p_iso_trans.y, p_iso_trans.z)
    }

    /// Inverse transforms a vector of points by the defined isometry
    /// # Attributes
    /// - `p_vec`: Vec of Point3 with Length components
    /// # Returns
    /// Returns the inverse-transformed point3s as Vec
    #[must_use]
    pub fn inverse_transform_points(&self, p_vec: &[Point3<Length>]) -> Vec<Point3<Length>> {
        p_vec
            .iter()
            .map(|p| self.inverse_transform_point(p))
            .collect::<Vec<Point3<Length>>>()
    }

    /// Transforms a single point by the defined isometry
    /// # Attributes
    /// - `p`: Point3 with f64 components, assuming a length in meter is specified
    /// # Returns
    /// Returns the transformed point3
    #[must_use]
    pub fn transform_point_f64(&self, p: &Point3<f64>) -> Point3<f64> {
        self.transform.transform_point(p)
    }

    /// Transforms a vector of points by the defined isometry
    /// # Attributes
    /// - `p_vec`: Vec of Point3 with f64 components, assuming a length in meter is specified
    /// # Returns
    /// Returns the transformed point3s as Vec
    #[must_use]
    pub fn transform_points_f64(&self, p_vec: &[Point3<f64>]) -> Vec<Point3<f64>> {
        p_vec
            .iter()
            .map(|p| self.transform_point_f64(p))
            .collect::<Vec<Point3<f64>>>()
    }

    /// Inverse transforms a single point by the defined isometry
    /// # Attributes
    /// - `p`: Point3 with f64 components, assuming a length in meter is specified
    /// # Returns
    /// Returns the inverse-transformed point3
    #[must_use]
    pub fn inverse_transform_point_f64(&self, p: &Point3<f64>) -> Point3<f64> {
        self.inverse.transform_point(p)
    }

    /// Inverse transforms a vector of points by the defined isometry
    /// # Attributes
    /// - `p_vec`: Vec of Point3 with Length components
    /// # Returns
    /// Returns the inverse-transformed point3s as Vec
    #[must_use]
    pub fn inverse_transform_points_f64(&self, p_vec: &[Point3<f64>]) -> Vec<Point3<f64>> {
        p_vec
            .iter()
            .map(|p| self.inverse_transform_point_f64(p))
            .collect::<Vec<Point3<f64>>>()
    }
    /// Transforms a single `Vector3<f64>` by the defined isometry
    /// # Attributes
    /// - `v`: Vector3 dfining a direction
    /// # Returns
    /// Returns the transformed `Vector3`
    #[must_use]
    pub fn transform_vector_f64(&self, v: &Vector3<f64>) -> Vector3<f64> {
        self.transform.transform_vector(v)
    }
    /// Transforms a vector of `Vector3<f64>` by the defined isometry
    /// # Attributes
    /// - `v_vec`: Vec of `Vector3`
    /// # Returns
    /// Returns the transformed `Vector3` as Vec
    #[must_use]
    pub fn transform_vectors_f64(&self, v_vec: &[Vector3<f64>]) -> Vec<Vector3<f64>> {
        v_vec
            .iter()
            .map(|p| self.transform_vector_f64(p))
            .collect::<Vec<Vector3<f64>>>()
    }
    /// Inverse transforms a single `Vector3<f64>` by the defined isometry
    /// # Attributes
    /// - `v`: `Vector3` defining a direction
    /// # Returns
    /// Returns the inverse-transformed `Vector3`
    #[must_use]
    pub fn inverse_transform_vector_f64(&self, v: &Vector3<f64>) -> Vector3<f64> {
        self.inverse.transform_vector(v)
    }
    /// Inverse transforms a vector of `Vector3<f64>` by the defined isometry
    /// # Attributes
    /// - `v_vec`: Vec of `Vector3`
    /// # Returns
    /// Returns the inverse-transformed `Vector3` as Vec
    #[must_use]
    pub fn inverse_transform_vectors_f64(&self, v_vec: &[Vector3<f64>]) -> Vec<Vector3<f64>> {
        v_vec
            .iter()
            .map(|p| self.inverse_transform_vector_f64(p))
            .collect::<Vec<Vector3<f64>>>()
    }
}
impl From<EnumProxy<Option<Isometry>>> for Proptype {
    fn from(value: EnumProxy<Option<Isometry>>) -> Self {
        Self::Isometry(value)
    }
}
impl From<Option<Isometry>> for Proptype {
    fn from(value: Option<Isometry>) -> Self {
        Self::Isometry(EnumProxy { value })
    }
}
#[cfg(feature = "bevy")]
#[allow(clippy::cast_possible_truncation)]
const fn as_f32(x: f64) -> f32 {
    x as f32
}
#[cfg(feature = "bevy")]
impl From<Isometry> for Transform {
    fn from(value: Isometry) -> Self {
        let t = value.transform.translation;
        let r = value.transform.rotation;
        Self::from_translation(Vec3::new(as_f32(t.x), as_f32(t.y), as_f32(t.z))).with_rotation(
            Quat::from_vec4(Vec4::new(
                as_f32(r.i),
                as_f32(r.j),
                as_f32(r.k),
                as_f32(r.w),
            )),
        )
    }
}
/// This function defines the coordinate axes on a plane.
/// This may be useful if points are projected onto that plane and should be represented by values of two coordinate axes that span the plane
/// If the plane normal is parallel to one of the main coordinate axes (x,y,z), the respective other axes are used.
/// Else if the plane normal is perpendicular to one of the main coordinate axes (x,y,z), this axis is used and the cross prouct with the plane normal and that axis
/// Otherwise, the first axis is chosen by by projecting the main axis onto that plane choosing the on with the largest angle to the plane normal. The Other one is again constructed via cross product
/// # Attributes
/// `plane_normal_vector`: normal vector of the plane
/// # Errors
/// This function errors if the plane normal vector has a zero length
pub fn define_plane_coordinate_axes_directions(
    plane_normal_vector: &Vector3<f64>,
) -> OpmResult<(Vector3<f64>, Vector3<f64>)> {
    if plane_normal_vector.norm() < f64::EPSILON {
        return Err(OpossumError::Other(
            "plane normal vector must have a non zero length!".into(),
        ));
    };
    //define the coordinate axes of the view onto the plane that is defined by the propagation axis as normal vector
    let (vec1, vec2) = if plane_normal_vector.cross(&Vector3::new(1., 0., 0.)).norm() < f64::EPSILON
    {
        //parallel to the x-axis: co_ax_1: z-axis / co_ax2: y-axis
        (Vector3::new(0., 0., 1.), Vector3::new(0., 1., 0.))
    } else if plane_normal_vector.cross(&Vector3::new(0., 1., 0.)).norm() < f64::EPSILON {
        //parallel to the y-axis: co_ax_1: z-axis / co_ax2: x-axis
        (Vector3::new(0., 0., 1.), Vector3::new(1., 0., 0.))
    } else if plane_normal_vector.cross(&Vector3::new(0., 0., 1.)).norm() < f64::EPSILON {
        //parallel to the z-axis: co_ax_1: x-axis / co_ax2: y-axis
        (Vector3::new(1., 0., 0.), Vector3::new(0., 1., 0.))
    } else if plane_normal_vector.dot(&Vector3::new(1., 0., 0.)) < f64::EPSILON {
        //propagation axis in yz plane
        let co_ax1 = Vector3::new(1., 0., 0.);
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else if plane_normal_vector.dot(&Vector3::new(0., 1., 0.)) < f64::EPSILON {
        //propagation axis in xz plane
        let co_ax1 = Vector3::new(0., 1., 0.);
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else if plane_normal_vector.dot(&Vector3::new(0., 0., 1.)) < f64::EPSILON {
        //propagation axis in xy plane
        let co_ax1 = Vector3::new(0., 0., 1.);
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else {
        //propagation axis is in neither of the cartesian coordinate planes
        //Choose the first coordinate axis by projecting the axes with the largest angle to the propagation axis onto the plane
        //the second one is defined by the cross product of the first axis and the propagation axis
        let p_zz_ang = plane_normal_vector.dot(&Vector3::new(0., 0., 1.)).acos();
        let p_yy_ang = plane_normal_vector.dot(&Vector3::new(0., 1., 0.)).acos();
        let p_xx_ang = plane_normal_vector.dot(&Vector3::new(1., 0., 0.)).acos();

        let mut co_ax1 = if p_zz_ang >= p_yy_ang && p_zz_ang >= p_xx_ang {
            plane_normal_vector
                - plane_normal_vector.dot(&Vector3::new(0., 0., 1.)) * Vector3::new(0., 0., 1.)
        } else if p_yy_ang >= p_zz_ang && p_yy_ang >= p_xx_ang {
            plane_normal_vector
                - plane_normal_vector.dot(&Vector3::new(0., 1., 0.)) * Vector3::new(0., 1., 0.)
        } else {
            plane_normal_vector
                - plane_normal_vector.dot(&Vector3::new(1., 0., 0.)) * Vector3::new(1., 0., 0.)
        };

        co_ax1 /= co_ax1.norm();

        (co_ax1, plane_normal_vector.cross(&co_ax1))
    };
    Ok((vec1, vec2))
}

/// Projects points onto a defined plane
/// # Attributes
/// `plane_normal_anchor`: anchor point that lies on the plane
/// `plane_normal_vector`: normal vector of the plane
/// # Errors
/// This function errors if the plane normal vector has a zero length or any of the provided input plane vectors includes a non finite entry
pub fn project_points_to_plane(
    plane_normal_anchor: &Vector3<f64>,
    plane_normal_vector: &Vector3<f64>,
    points_to_project: &[Vector3<f64>],
) -> OpmResult<MatrixXx3<f64>> {
    if relative_eq!(plane_normal_vector.norm(), 0.0)
        || plane_normal_vector.iter().any(|x| !(*x).is_finite())
    {
        return Err(OpossumError::Other(
            "plane normal vector must have a non zero length and be finite!".into(),
        ));
    };
    if plane_normal_anchor.iter().any(|x| !f64::is_finite(*x)) {
        return Err(OpossumError::Other(
            "plane normal anchor must be finite!".into(),
        ));
    };

    let mut pos_projection = MatrixXx3::<f64>::zeros(points_to_project.len());
    for (row, pos) in points_to_project.iter().enumerate() {
        let normed_normal_vec = plane_normal_vector / plane_normal_vector.norm();
        let displacement_vector = pos - plane_normal_anchor;

        let projection = plane_normal_anchor + displacement_vector
            - displacement_vector.dot(&normed_normal_vec) * normed_normal_vec;

        pos_projection.set_row(row, &projection.transpose());
    }
    Ok(pos_projection)
}

/// Projects points onto a defined plane and represents their position as combination of distances along the base vectors of that plane.
/// If both base vectors are None, `define_plane_coordinate_axes_directions` is used to define these axes. If only one of them is None, the cross product of the defined axis and the plane normal is used.
/// # Attributes
/// `plane_normal_anchor`: anchor point that lies on the plane
/// `plane_normal_vector`: normal vector of the plane
/// `plane_base_vec_1_opt`: first base vector of the plane.
/// `plane_base_vec_2_opt`: second base vector of the plane
/// # Errors
/// This function errors if the plane normal vector has a zero length or any of the provided input plane vectors includes a non `finite_vector`
pub fn project_pos_to_plane_with_base_vectors(
    plane_normal_anchor: &Vector3<f64>,
    plane_normal_vector: &Vector3<f64>,
    plane_base_vec_1_opt: Option<&Vector3<f64>>,
    plane_base_vec_2_opt: Option<&Vector3<f64>>,
    points_to_project: &[Vector3<f64>],
) -> OpmResult<MatrixXx2<f64>> {
    if relative_eq!(plane_normal_vector.norm(), 0.0)
        || plane_normal_vector.iter().any(|x| !f64::is_finite(*x))
    {
        return Err(OpossumError::Other(
            "plane normal vector must have a non zero length and be finite!".into(),
        ));
    };
    if plane_normal_anchor.iter().any(|x| !f64::is_finite(*x)) {
        return Err(OpossumError::Other(
            "plane normal anchor must be finite!".into(),
        ));
    };

    let (plane_co_ax_1, plane_co_ax_2) = if let (Some(plane_base_vec_1), Some(plane_base_vec_2)) =
        (plane_base_vec_1_opt, plane_base_vec_2_opt)
    {
        (*plane_base_vec_1, *plane_base_vec_2)
    } else if let Some(plane_base_vec_1) = plane_base_vec_1_opt {
        (
            plane_normal_vector.cross(plane_base_vec_1),
            *plane_base_vec_1,
        )
    } else if let Some(plane_base_vec_2) = plane_base_vec_2_opt {
        (
            plane_normal_vector.cross(plane_base_vec_2),
            *plane_base_vec_2,
        )
    } else {
        define_plane_coordinate_axes_directions(plane_normal_vector)?
    };

    if plane_co_ax_1.iter().any(|x| !f64::is_finite(*x))
        || plane_co_ax_2.iter().any(|x| !f64::is_finite(*x))
    {
        return Err(OpossumError::Other(
            "base vector of the plane contains non-finite values!".into(),
        ));
    };

    if relative_eq!(plane_co_ax_1.norm(), 0.0) || relative_eq!(plane_co_ax_2.norm(), 0.0) {
        return Err(OpossumError::Other(
            "base vector of the plane has a length of zero!".into(),
        ));
    };

    let mut pos_projection = MatrixXx2::<f64>::zeros(points_to_project.len());
    for (row, pos) in points_to_project.iter().enumerate() {
        let closest_to_axis_vec = pos
            - plane_normal_anchor
            - (pos - plane_normal_anchor).dot(plane_normal_vector) * plane_normal_vector;

        pos_projection[(row, 0)] = closest_to_axis_vec.dot(&plane_co_ax_1);
        pos_projection[(row, 1)] = closest_to_axis_vec.dot(&plane_co_ax_2);
    }
    Ok(pos_projection)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    use approx::assert_relative_eq;
    #[test]
    fn new_along_z() {
        assert!(Isometry::new_along_z(millimeter!(f64::NAN)).is_err());
        assert!(Isometry::new_along_z(millimeter!(f64::INFINITY)).is_err());
        assert!(Isometry::new_along_z(millimeter!(f64::NEG_INFINITY)).is_err());
        let i = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        assert_eq!(i.transform.translation.x, 0.0);
        assert_eq!(i.transform.translation.y, 0.0);
        assert_eq!(i.transform.translation.z, 0.01);
        assert_eq!(i.transform.rotation.i, 0.0);
        assert_eq!(i.transform.rotation.j, 0.0);
        assert_eq!(i.transform.rotation.k, 0.0);
    }
    #[test]
    fn append_z() {
        let i1 = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        let i2 = Isometry::new_along_z(millimeter!(20.0)).unwrap();
        let i = i1.append(&i2);
        assert_eq!(i.transform.translation.x, 0.0);
        assert_eq!(i.transform.translation.y, 0.0);
        assert_eq!(i.transform.translation.z, 0.03);
    }
    #[test]
    fn append_with_rot() {
        let i1 = Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 90.0, 0.0)).unwrap();
        let i2 = Isometry::new_along_z(millimeter!(20.0)).unwrap();
        let i = i1.append(&i2);
        let new_point = i.transform_point_f64(&Point3::origin());
        assert_relative_eq!(new_point, Point3::new(0.02, 0.0, 0.01));
    }
    #[test]
    fn define_plane_coordinate_axes_directions_test() {
        assert!(define_plane_coordinate_axes_directions(&Vector3::new(0., 0., 0.)).is_err());

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 0., 0.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(0., 0., 1.));
        assert_relative_eq!(ax2, Vector3::new(0., 1., 0.));

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(0., 1., 0.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(0., 0., 1.));
        assert_relative_eq!(ax2, Vector3::new(1., 0., 0.));

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(0., 0., 1.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(1., 0., 0.));
        assert_relative_eq!(ax2, Vector3::new(0., 1., 0.));

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(0., 1., 1.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(1., 0., 0.));
        assert_relative_eq!(
            ax2,
            Vector3::new(0., 1., 1.).cross(&Vector3::new(1., 0., 0.))
        );

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 0., 1.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(0., 1., 0.));
        assert_relative_eq!(
            ax2,
            Vector3::new(1., 0., 1.).cross(&Vector3::new(0., 1., 0.))
        );

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 1., 0.)).unwrap();
        assert_relative_eq!(ax1, Vector3::new(0., 0., 1.));
        assert_relative_eq!(
            ax2,
            Vector3::new(1., 1., 0.).cross(&Vector3::new(0., 0., 1.))
        );

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 0.1, 0.1)).unwrap();
        assert_relative_eq!(
            ax1,
            Vector3::new(0.9950371902099893, 0.09950371902099893, 0.0)
        );
        assert_relative_eq!(
            ax2,
            Vector3::new(-0.009950371902099894, 0.09950371902099893, 0.0)
        );
    }
    #[test]
    fn project_points_to_plane_test() {
        let pos = project_points_to_plane(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)],
        )
        .unwrap();

        assert_relative_eq!(pos[(0, 0)], 0.);
        assert_relative_eq!(pos[(0, 1)], 0.);
        assert_relative_eq!(pos[(1, 0)], 10.);
        assert_relative_eq!(pos[(1, 1)], 1.);

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 0.),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., f64::NAN),
            &Vector3::new(0., 0., 1.),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., f64::INFINITY),
            &Vector3::new(0., 0., 1.),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., f64::NEG_INFINITY),
            &Vector3::new(0., 0., 1.),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., 1.),
            &Vector3::new(0., 1., f64::NAN),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., 1.),
            &Vector3::new(0., 1., f64::INFINITY),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());

        assert!(project_points_to_plane(
            &Vector3::new(0., 0., 1.),
            &Vector3::new(0., 1., f64::NEG_INFINITY),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
        )
        .is_err());
    }
    #[test]
    fn project_pos_to_plane_with_base_vectors_test() {
        let projection = project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        );
        assert!(projection.is_ok());

        let proj = projection.unwrap();
        assert_relative_eq!(proj[(0, 0)], 0.);
        assert_relative_eq!(proj[(0, 1)], 0.);
        assert_relative_eq!(proj[(1, 0)], 10.);
        assert_relative_eq!(proj[(1, 1)], 0.);

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 0.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., f64::NAN),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., f64::INFINITY),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., f64::NEG_INFINITY),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(0., 0., 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., 0., 0.)),
            Some(&Vector3::new(0., 0., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., f64::NAN, 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., f64::INFINITY, 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(1., f64::NEG_INFINITY, 0.)),
            Some(&Vector3::new(0., 1., 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(0., 1., 0.)),
            Some(&Vector3::new(1., f64::NAN, 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(0., 1., 0.)),
            Some(&Vector3::new(1., f64::INFINITY, 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());

        assert!(project_pos_to_plane_with_base_vectors(
            &Vector3::new(0., 0., 0.),
            &Vector3::new(0., 0., 1.),
            Some(&Vector3::new(0., 1., 0.)),
            Some(&Vector3::new(1., f64::NEG_INFINITY, 0.)),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        )
        .is_err());
    }
}
