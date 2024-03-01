//!for all the functions, structs or trait that may be used for geometrical transformations
#![warn(missing_docs)]
use nalgebra::{MatrixXx2, MatrixXx3, Vector3};

use crate::error::{OpmResult, OpossumError};
/// This function defines the coordinate axes on a plane.
/// This may be useful if points are projected onto that plane and should be represented by values of two coordinate axes that span the plane
/// If the plane normal is parallel to one of the main coordinate axes (x,y,z), the respective other axes are used.
/// Else if the plane normal is perpendicular to one of the main coordinate axes (x,y,z), this axis is used and the cross prouct with the plane normal and that axis
/// Otherwise, the first axis is chosen by by projecting the main axis onto that plane choosing the on with the largest angle to the plane normal. The Other one is again constructed via cross product
/// # Attributes
/// `plane_normal_vector`: normal vector of the plane
/// # Errors
/// This function errors if the plane normal vector has a zero length
#[must_use]
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
#[must_use]
pub fn project_points_to_plane(
    plane_normal_anchor: &Vector3<f64>,
    plane_normal_vector: &Vector3<f64>,
    points_to_project: &[Vector3<f64>],
) -> MatrixXx3<f64> {
    let mut pos_projection = MatrixXx3::<f64>::zeros(points_to_project.len());
    for (row, pos) in points_to_project.iter().enumerate() {
        let closest_to_axis_vec = pos
            - plane_normal_anchor
            - (pos - plane_normal_anchor).dot(plane_normal_vector) * plane_normal_vector;

        pos_projection.set_row(
            row,
            &(plane_normal_anchor - closest_to_axis_vec).transpose(),
        );
    }
    pos_projection
}

/// Projects points onto a defined plane and represents their position as combination of distances along the base vectors of that plane.
/// If both base vectors are None, `define_plane_coordinate_axes_directions` is used to define these axes. If only one of them is None, the cross product of the defined axis and the plane normal is used.
/// # Attributes
/// `plane_normal_anchor`: anchor point that lies on the plane
/// `plane_normal_vector`: normal vector of the plane
/// `plane_base_vec_1_opt`: first base vector of the plane.
/// `plane_base_vec_2_opt`: second base vector of the plane
/// # Errors
/// This function errors if the plane normal vector has a zero length
#[must_use]
pub fn project_pos_to_plane_with_base_vectors(
    plane_normal_anchor: &Vector3<f64>,
    plane_normal_vector: &Vector3<f64>,
    plane_base_vec_1_opt: Option<&Vector3<f64>>,
    plane_base_vec_2_opt: Option<&Vector3<f64>>,
    points_to_project: &[Vector3<f64>],
) -> OpmResult<MatrixXx2<f64>> {
    if plane_normal_vector.norm() < f64::EPSILON {
        return Err(OpossumError::Other(
            "plane normal vector must have a non zero length!".into(),
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
    #[test]
    fn create_projection_coordinate_axes_directions_test() {
        todo!()
    }
    fn project_pos_to_plane_test() {
        todo!()
    }
    fn project_pos_to_plane_with_base_vectors_test() {
        todo!()
    }
}
