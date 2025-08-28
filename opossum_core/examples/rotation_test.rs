use nalgebra::{Isometry3, Point3, Rotation, Vector3};
use opossum::{degree, utils::geom_transformation::Isometry};

fn main() {
    let rot_x = Isometry3::rotation(Vector3::x() * std::f64::consts::PI / 4.);
    let rot_y = Isometry3::rotation(Vector3::y() * std::f64::consts::PI / 4.);
    let rot_from_euler =
        Rotation::from_euler_angles(std::f64::consts::PI / 4., std::f64::consts::PI / 4., 0.);
    let rot_xy: Isometry = Isometry::new(Point3::origin(), degree!(45., 45., 0.)).unwrap();

    let start_vec = Vector3::new(0., 0., 1.);

    let end_vec_x_then_y = rot_y * rot_x * start_vec;
    let end_vec_xy = rot_xy.transform_vector_f64(&start_vec);
    let end_vec_euler = rot_from_euler * start_vec;

    println!("{end_vec_x_then_y}");
    println!("{end_vec_xy}");
    println!("{end_vec_euler}");
}
