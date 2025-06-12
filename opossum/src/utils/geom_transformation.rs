//!for all the functions, structs or trait that may be used for geometrical transformations
#![warn(missing_docs)]
use std::fmt::Display;

use crate::{
    degree,
    error::{OpmResult, OpossumError},
    meter,
    properties::Proptype,
    radian,
};
use approx::relative_eq;
use nalgebra::{
    Isometry3, MatrixXx2, MatrixXx3, Point3, Rotation3, Translation, Translation3, Vector3, vector,
};
use num::Zero;
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, Visitor},
};
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
    length::meter,
};

/// Struct to store the isometric transofmeation matrix and its inverse
#[derive(Debug, Clone, Default, Serialize, PartialEq, Copy)]
pub struct Isometry {
    transform: Isometry3<f64>,
    #[serde(skip_serializing)]
    inverse: Isometry3<f64>,
}
impl Isometry {
    /// Creates a new [`Isometry`] which stores the rotation and translation as a transform matrix and its inverse.
    /// Internally, translation is handled in meter, rotation in radians
    /// # Attributes
    /// - `translation`: vector of translation for each axis as [`Length`]
    /// - `axes_angles`: rotation [`Angle`]s for each axis
    ///   Note: the rotation is applied in the order x -> y -> z
    ///
    /// # Errors
    /// his function return an error if the
    ///  - the translation coordinates are not finite
    ///  - the axis angles are not finite
    pub fn new(translation: Point3<Length>, axes_angles: Point3<Angle>) -> OpmResult<Self> {
        if translation.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "translation coordinates must be finite".into(),
            ));
        }
        if axes_angles.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other("axis angles must be finite".into()));
        }
        let trans_in_m = Vector3::from_vec(
            translation
                .iter()
                .map(Length::get::<meter>)
                .collect::<Vec<f64>>(),
        );
        let rot_in_radian = Vector3::from_vec(
            axes_angles
                .iter()
                .map(Angle::get::<radian>)
                .collect::<Vec<f64>>(),
        );
        let translation_iso = Translation3::new(trans_in_m[0], trans_in_m[1], trans_in_m[2]);
        let rotation_iso =
            Rotation3::from_euler_angles(rot_in_radian[0], rot_in_radian[1], rot_in_radian[2]);
        Ok(Self::new_from_transform(Isometry3::from_parts(
            translation_iso,
            rotation_iso.into(),
        )))
    }

    ///Returns the transform matrix of this [`Isometry`]
    #[must_use]
    pub const fn get_transform(&self) -> Isometry3<f64> {
        self.transform
    }
    ///Returns the inverse transform matrix of this [`Isometry`]
    #[must_use]
    pub const fn get_inv_transform(&self) -> Isometry3<f64> {
        self.inverse
    }

    /// Creates a new translation [`Isometry`]
    ///
    /// Internally, translation is handled in meter
    /// # Attributes
    /// - `translation`: vector of translation for each axis as [`Length`]
    ///
    /// # Errors
    /// his function return an error if the
    ///  - the translation coordinates are not finite
    pub fn new_translation(translation: Point3<Length>) -> OpmResult<Self> {
        if translation.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other(
                "translation coordinates must be finite".into(),
            ));
        }
        let trans_in_m = Vector3::from_vec(
            translation
                .iter()
                .map(Length::get::<meter>)
                .collect::<Vec<f64>>(),
        );
        let translation_iso = Translation3::new(trans_in_m[0], trans_in_m[1], trans_in_m[2]);

        Ok(Self::new_from_transform(translation_iso.into()))
    }
    /// Creates a new rotation [`Isometry`]
    ///
    /// Internally, rotation is handled in radians
    /// # Attributes
    /// - `axes_angles`: rotation [`Angle`]s for each axis
    ///   Note: the rotation is applied in the order x -> y -> z
    ///
    /// # Errors
    /// his function return an error if the
    ///  - the axis angles are not finite
    pub fn new_rotation(axes_angles: Point3<Angle>) -> OpmResult<Self> {
        if axes_angles.iter().any(|x| !x.is_finite()) {
            return Err(OpossumError::Other("axis angles must be finite".into()));
        }
        let rot_in_radian = Vector3::from_vec(
            axes_angles
                .iter()
                .map(Angle::get::<radian>)
                .collect::<Vec<f64>>(),
        );
        let rotation_iso =
            Rotation3::from_euler_angles(rot_in_radian[0], rot_in_radian[1], rot_in_radian[2]);

        Ok(Self::new_from_transform(Isometry3::from_parts(
            Translation3::identity(),
            rotation_iso.into(),
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
        vector![meter!(t.x), meter!(t.y), meter!(t.z)]
    }
    /// Returns the translation of this [`Isometry`].
    #[must_use]
    pub fn translation(&self) -> Point3<Length> {
        let t = self.transform.translation;
        meter!(t.x, t.y, t.z)
    }
    /// Returns the translation on a specific axis of this [`Isometry`].
    #[must_use]
    pub fn translation_of_axis(&self, axis: TranslationAxis) -> Length {
        let t = self.translation();
        match axis {
            TranslationAxis::X => t.x,
            TranslationAxis::Y => t.y,
            TranslationAxis::Z => t.z,
        }
    }
    /// Sets a value on the translation axis of this [`Isometry`].
    #[must_use]
    pub fn set_translation_of_axis(
        &mut self,
        axis: TranslationAxis,
        value: Length,
    ) -> OpmResult<()> {
        let mut new_trans = self.translation();
        let rot = self.rotation();
        match axis {
            TranslationAxis::X => new_trans.x = value,
            TranslationAxis::Y => new_trans.y = value,
            TranslationAxis::Z => new_trans.z = value,
        }
        *self = Isometry::new(new_trans, rot)?;
        Ok(())
    }
    /// Returns the rotation of this [`Isometry`].
    #[must_use]
    pub fn rotation(&self) -> Point3<Angle> {
        let rot = self.transform.rotation.euler_angles();
        radian!(rot.0, rot.1, rot.2)
    }
    /// Returns the rotation angle around a specific axis of this [`Isometry`].
    #[must_use]
    pub fn rotation_of_axis(&self, axis: RotationAxis) -> Angle {
        let r = self.rotation();
        match axis {
            RotationAxis::Roll => r.x,
            RotationAxis::Pitch => r.y,
            RotationAxis::Yaw => r.z,
        }
    }
    /// Sets a value on the rotation axis of this [`Isometry`].
    #[must_use]
    pub fn set_rotation_of_axis(&mut self, axis: RotationAxis, value: Angle) -> OpmResult<()> {
        let trans = self.translation();
        let mut new_rot = self.rotation();
        match axis {
            RotationAxis::Roll => new_rot.x = value,
            RotationAxis::Pitch => new_rot.y = value,
            RotationAxis::Yaw => new_rot.z = value,
        }
        *self = Isometry::new(trans, new_rot)?;
        Ok(())
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

/// Define the translation and rotation axes for the [`Isometry`]
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum TranslationAxis {
    /// The X axis
    X,
    /// The Y axis
    Y,
    /// The Z axis
    Z,
}

/// Define the rotation axes for the [`Isometry`]
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum RotationAxis {
    /// The Roll axis
    Roll,
    /// The Pitch axis
    Pitch,
    /// The Yaw axis
    Yaw,
}

impl Display for TranslationAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationAxis::X => write!(f, "X"),
            TranslationAxis::Y => write!(f, "Y"),
            TranslationAxis::Z => write!(f, "Z"),
        }
    }
}

impl Display for RotationAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationAxis::Roll => write!(f, "Roll"),
            RotationAxis::Pitch => write!(f, "Pitch"),
            RotationAxis::Yaw => write!(f, "Yaw"),
        }
    }
}

impl Display for Isometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = Length::format_args(meter, uom::fmt::DisplayStyle::Abbreviation);
        let deg = Angle::format_args(uom::si::angle::degree, uom::fmt::DisplayStyle::Abbreviation);
        let trans = self.translation();
        let rot = self.rotation();
        write!(
            f,
            "translation: ({:.3}, {:.3}, {:.3}), rotation: ({:.3}, {:.3}, {:.3})",
            m.with(trans[0]),
            m.with(trans[1]),
            m.with(trans[2]),
            deg.with(rot[0]),
            deg.with(rot[1]),
            deg.with(rot[2]),
        )
    }
}
impl From<Option<Isometry>> for Proptype {
    fn from(value: Option<Isometry>) -> Self {
        Self::Isometry(value)
    }
}
/// Custom deserializer for [`Isometry`]
///
/// This is necessary since only the `transform` field need to be serialized and deserialized while the
/// `inverse` field is automatically calculated during deserialization.
impl<'de> Deserialize<'de> for Isometry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        enum Field {
            Transform,
        }
        const FIELDS: &[&str] = &["transform"];

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl Visitor<'_> for FieldVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("`transform`")
                    }
                    fn visit_str<E>(self, value: &str) -> std::result::Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "transform" => Ok(Field::Transform),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct OpticRefVisitor;

        impl<'de> Visitor<'de> for OpticRefVisitor {
            type Value = Isometry;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a struct OpticRef")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Isometry, A::Error>
            where
                A: MapAccess<'de>,
            {
                // let mut node_type = None;
                let mut transform = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Transform => {
                            if transform.is_some() {
                                return Err(de::Error::duplicate_field("attributes"));
                            }
                            transform = Some(map.next_value::<Isometry3<f64>>()?);
                        }
                    }
                }

                let transform = transform.ok_or_else(|| de::Error::missing_field("transform"))?;
                let iso = Isometry {
                    transform,
                    inverse: transform.inverse(),
                };
                Ok(iso)
            }
        }
        deserializer.deserialize_struct("OpticRef", FIELDS, OpticRefVisitor)
    }
}
/// This function defines the coordinate axes on a plane.
///
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
    }
    //define the coordinate axes of the view onto the plane that is defined by the propagation axis as normal vector
    let (vec1, vec2) = if plane_normal_vector.cross(&Vector3::x()).norm() < f64::EPSILON {
        //parallel to the x-axis: co_ax_1: z-axis / co_ax2: y-axis
        (Vector3::z(), Vector3::y())
    } else if plane_normal_vector.cross(&Vector3::y()).norm() < f64::EPSILON {
        //parallel to the y-axis: co_ax_1: z-axis / co_ax2: x-axis
        (Vector3::z(), Vector3::x())
    } else if plane_normal_vector.cross(&Vector3::z()).norm() < f64::EPSILON {
        //parallel to the z-axis: co_ax_1: x-axis / co_ax2: y-axis
        (Vector3::x(), Vector3::y())
    } else if plane_normal_vector.dot(&Vector3::x()) < f64::EPSILON {
        //propagation axis in yz plane
        let co_ax1 = Vector3::x();
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else if plane_normal_vector.dot(&Vector3::y()) < f64::EPSILON {
        //propagation axis in xz plane
        let co_ax1 = Vector3::y();
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else if plane_normal_vector.dot(&Vector3::z()) < f64::EPSILON {
        //propagation axis in xy plane
        let co_ax1 = Vector3::z();
        (co_ax1, plane_normal_vector.cross(&co_ax1))
    } else {
        //propagation axis is in neither of the cartesian coordinate planes
        //Choose the first coordinate axis by projecting the axes with the largest angle to the propagation axis onto the plane
        //the second one is defined by the cross product of the first axis and the propagation axis
        let p_zz_ang = plane_normal_vector.dot(&Vector3::z()).acos();
        let p_yy_ang = plane_normal_vector.dot(&Vector3::y()).acos();
        let p_xx_ang = plane_normal_vector.dot(&Vector3::x()).acos();

        let mut co_ax1 = if p_zz_ang >= p_yy_ang && p_zz_ang >= p_xx_ang {
            plane_normal_vector - plane_normal_vector.dot(&Vector3::z()) * Vector3::z()
        } else if p_yy_ang >= p_zz_ang && p_yy_ang >= p_xx_ang {
            plane_normal_vector - plane_normal_vector.dot(&Vector3::y()) * Vector3::y()
        } else {
            plane_normal_vector - plane_normal_vector.dot(&Vector3::x()) * Vector3::x()
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
    }
    if plane_normal_anchor.iter().any(|x| !f64::is_finite(*x)) {
        return Err(OpossumError::Other(
            "plane normal anchor must be finite!".into(),
        ));
    }
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

/// Projects points onto a defined plane
///
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
    }
    if plane_normal_anchor.iter().any(|x| !f64::is_finite(*x)) {
        return Err(OpossumError::Other(
            "plane normal anchor must be finite!".into(),
        ));
    }
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
    }
    if relative_eq!(plane_co_ax_1.norm(), 0.0) || relative_eq!(plane_co_ax_2.norm(), 0.0) {
        return Err(OpossumError::Other(
            "base vector of the plane has a length of zero!".into(),
        ));
    }
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
    use core::f64;

    use super::*;
    use crate::millimeter;
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use assert_matches::assert_matches;
    #[test]
    fn display() {
        let i = Isometry::identity();
        assert_eq!(
            format!("{i}"),
            "translation: (0.000 m, 0.000 m, 0.000 m), rotation: (0.000 °, -0.000 °, 0.000 °)"
        );
    }
    #[test]
    fn new() {
        let inf_vals = vec![f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

        for val in &inf_vals {
            assert!(Isometry::new(millimeter!(*val, 0., 0.), degree!(0., 0., 0.)).is_err());
            assert!(Isometry::new(millimeter!(0., *val, 0.), degree!(0., 0., 0.)).is_err());
            assert!(Isometry::new(millimeter!(0., 0., *val), degree!(0., 0., 0.)).is_err());

            assert!(Isometry::new(millimeter!(0., 0., 0.), degree!(*val, 0., 0.)).is_err());
            assert!(Isometry::new(millimeter!(0., 0., 0.), degree!(0., *val, 0.)).is_err());
            assert!(Isometry::new(millimeter!(0., 0., 0.), degree!(0., 0., *val)).is_err());
        }
    }
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
    fn new_translation() {
        assert!(Isometry::new_translation(meter!(f64::NAN, 0.0, 0.0)).is_err());
        assert!(Isometry::new_translation(meter!(f64::NEG_INFINITY, 0.0, 0.0)).is_err());
        assert!(Isometry::new_translation(meter!(f64::INFINITY, 0.0, 0.0)).is_err());

        assert!(Isometry::new_translation(meter!(0.0, f64::NAN, 0.0)).is_err());
        assert!(Isometry::new_translation(meter!(0.0, f64::NEG_INFINITY, 0.0)).is_err());
        assert!(Isometry::new_translation(meter!(0.0, f64::INFINITY, 0.0)).is_err());

        assert!(Isometry::new_translation(meter!(0.0, 0.0, f64::NAN)).is_err());
        assert!(Isometry::new_translation(meter!(0.0, 0.0, f64::NEG_INFINITY)).is_err());
        assert!(Isometry::new_translation(meter!(0.0, 0.0, f64::INFINITY)).is_err());

        let i = Isometry::new_translation(meter!(1.0, 2.0, 3.0)).unwrap();
        assert_eq!(
            i.transform,
            Isometry3::<f64>::new(vector![1.0, 2.0, 3.0], vector![0.0, 0.0, 0.0])
        );
    }
    #[test]
    fn new_rotation() {
        assert!(Isometry::new_rotation(degree!(f64::NAN, 0.0, 0.0)).is_err());
        assert!(Isometry::new_rotation(degree!(f64::NEG_INFINITY, 0.0, 0.0)).is_err());
        assert!(Isometry::new_rotation(degree!(f64::INFINITY, 0.0, 0.0)).is_err());

        assert!(Isometry::new_rotation(degree!(0.0, f64::NAN, 0.0)).is_err());
        assert!(Isometry::new_rotation(degree!(0.0, f64::NEG_INFINITY, 0.0)).is_err());
        assert!(Isometry::new_rotation(degree!(0.0, f64::INFINITY, 0.0)).is_err());

        assert!(Isometry::new_rotation(degree!(0.0, 0.0, f64::NAN)).is_err());
        assert!(Isometry::new_rotation(degree!(0.0, 0.0, f64::NEG_INFINITY)).is_err());
        assert!(Isometry::new_rotation(degree!(0.0, 0.0, f64::INFINITY)).is_err());
    }
    #[test]
    fn get_transform() {
        let i = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        assert_eq!(
            i.get_transform(),
            Isometry3::<f64>::new(vector!(0.0, 0.0, 0.01), vector!(0.0, 0.0, 0.0))
        );
    }
    #[test]
    fn get_inv_transform() {
        let i = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        assert_eq!(
            i.get_inv_transform(),
            Isometry3::<f64>::new(vector!(0.0, 0.0, -0.01), vector!(0.0, 0.0, 0.0))
        );
    }
    #[test]
    fn translation_vec() {
        let i = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        assert_eq!(
            i.translation_vec(),
            vector![millimeter!(0.0), millimeter!(0.0), millimeter!(10.0)]
        );
    }
    #[test]
    fn translation() {
        let i = Isometry::new_along_z(millimeter!(10.0)).unwrap();
        assert_eq!(i.translation(), millimeter!(0.0, 0.0, 10.0));
    }
    #[test]
    fn rotation() {
        let i = Isometry::new(millimeter!(0.0, 0.0, 0.0), degree!(10.0, 20.0, 30.0)).unwrap();
        let rot = i.rotation();
        assert_abs_diff_eq!(rot[0].value, degree!(10.0).value);
        assert_abs_diff_eq!(rot[1].value, degree!(20.0).value);
        assert_abs_diff_eq!(rot[2].value, degree!(30.0).value);
    }
    #[test]
    fn identity() {
        let i = Isometry::identity();
        assert_eq!(i.transform, nalgebra::Isometry::identity());
        assert_eq!(i.inverse, nalgebra::Isometry::identity());
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
        assert!(define_plane_coordinate_axes_directions(&Vector3::zeros()).is_err());

        let (ax1, ax2) = define_plane_coordinate_axes_directions(&Vector3::x()).unwrap();
        assert_relative_eq!(ax1, Vector3::z());
        assert_relative_eq!(ax2, Vector3::y());

        let (ax1, ax2) = define_plane_coordinate_axes_directions(&Vector3::y()).unwrap();
        assert_relative_eq!(ax1, Vector3::z());
        assert_relative_eq!(ax2, Vector3::x());

        let (ax1, ax2) = define_plane_coordinate_axes_directions(&Vector3::z()).unwrap();
        assert_relative_eq!(ax1, Vector3::x());
        assert_relative_eq!(ax2, Vector3::y());

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(0., 1., 1.)).unwrap();
        assert_relative_eq!(ax1, Vector3::x());
        assert_relative_eq!(ax2, Vector3::new(0., 1., 1.).cross(&Vector3::x()));

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 0., 1.)).unwrap();
        assert_relative_eq!(ax1, Vector3::y());
        assert_relative_eq!(ax2, Vector3::new(1., 0., 1.).cross(&Vector3::y()));

        let (ax1, ax2) =
            define_plane_coordinate_axes_directions(&Vector3::new(1., 1., 0.)).unwrap();
        assert_relative_eq!(ax1, Vector3::z());
        assert_relative_eq!(ax2, Vector3::new(1., 1., 0.).cross(&Vector3::z()));

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
            &Vector3::zeros(),
            &Vector3::z(),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)],
        )
        .unwrap();

        assert_relative_eq!(pos[(0, 0)], 0.);
        assert_relative_eq!(pos[(0, 1)], 0.);
        assert_relative_eq!(pos[(1, 0)], 10.);
        assert_relative_eq!(pos[(1, 1)], 1.);

        assert!(
            project_points_to_plane(
                &Vector3::zeros(),
                &Vector3::zeros(),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::new(0., 0., f64::NAN),
                &Vector3::z(),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::new(0., 0., f64::INFINITY),
                &Vector3::z(),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::new(0., 0., f64::NEG_INFINITY),
                &Vector3::z(),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::z(),
                &Vector3::new(0., 1., f64::NAN),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::z(),
                &Vector3::new(0., 1., f64::INFINITY),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );

        assert!(
            project_points_to_plane(
                &Vector3::z(),
                &Vector3::new(0., 1., f64::NEG_INFINITY),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 1., 3.)]
            )
            .is_err()
        );
    }
    #[test]
    fn project_pos_to_plane_with_base_vectors_test() {
        let projection = project_pos_to_plane_with_base_vectors(
            &Vector3::zeros(),
            &Vector3::z(),
            Some(&Vector3::x()),
            Some(&Vector3::y()),
            &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
        );
        assert!(projection.is_ok());

        let proj = projection.unwrap();
        assert_relative_eq!(proj[(0, 0)], 0.);
        assert_relative_eq!(proj[(0, 1)], 0.);
        assert_relative_eq!(proj[(1, 0)], 10.);
        assert_relative_eq!(proj[(1, 1)], 0.);

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::zeros(),
                Some(&Vector3::x()),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::new(0., 0., f64::NAN),
                &Vector3::z(),
                Some(&Vector3::x()),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::new(0., 0., f64::INFINITY),
                &Vector3::z(),
                Some(&Vector3::x()),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::new(0., 0., f64::NEG_INFINITY),
                &Vector3::z(),
                Some(&Vector3::x()),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::zeros()),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::x()),
                Some(&Vector3::zeros()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::new(1., f64::NAN, 0.)),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::new(1., f64::INFINITY, 0.)),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::new(1., f64::NEG_INFINITY, 0.)),
                Some(&Vector3::y()),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::y()),
                Some(&Vector3::new(1., f64::NAN, 0.)),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::y()),
                Some(&Vector3::new(1., f64::INFINITY, 0.)),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );

        assert!(
            project_pos_to_plane_with_base_vectors(
                &Vector3::zeros(),
                &Vector3::z(),
                Some(&Vector3::y()),
                Some(&Vector3::new(1., f64::NEG_INFINITY, 0.)),
                &[Vector3::new(0., 0., -4.), Vector3::new(10., 0., 3.)],
            )
            .is_err()
        );
    }
    #[test]
    fn from() {
        assert_matches!(Some(Isometry::identity()).into(), Proptype::Isometry(_));
    }
}
