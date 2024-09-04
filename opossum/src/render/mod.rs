mod sdf;
pub use sdf::{Renderable, SDFCollection, SDFOperation, SDF};

use crate::{
    error::{OpmResult, OpossumError}, surface::Surf, utils::{geom_transformation::Isometry, griddata::linspace}
};
use itertools::Itertools;
use nalgebra::{Point2, Point3, Vector2, Vector3};
use num::ToPrimitive;
use plotters::{
    backend::BitMapBackend, chart::ChartBuilder, drawing::IntoDrawingArea, element::BitMapElement,
    style::WHITE,
};
use rayon::prelude::*;
use uom::si::{
    angle::radian,
    f64::{Angle, Length},
};

const MIN_DIST: f64 = 1e-4;
const MAX_DIST: f64 = 1e1;

pub trait Color {
    fn get_color(&self, p: &Point3<f64>) -> Vector3<f64>;
}
pub trait Render<'a>: SDF + Sync {
    /// Plot a rendered image
    /// # Errors
    /// This function errors if the image data length and the image shape doe snot match
    fn plot_image(&self, image_dat: &[f64], image_shape: (u32, u32), fpath: &str) -> OpmResult<()> {
        if (image_shape.0 * image_shape.1 * 3) as usize != image_dat.len() {
            return Err(OpossumError::Other(
                "Shape of image and vector length does not match! Cannot render image!".into(),
            ));
        }
        let root = BitMapBackend::new(fpath, image_shape).into_drawing_area();
        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(&root)
            .build_cartesian_2d(0.0..1.0, 0.0..1.0)
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            .disable_y_axis()
            .disable_x_axis()
            .draw()
            .unwrap();
        let u8_image_dat = image_dat
            .iter()
            .map(|x| (x.abs() * 255.).to_u8().unwrap())
            .collect_vec();

        let elem_opt = BitMapElement::with_owned_buffer((0., 1.0), image_shape, u8_image_dat);

        if let Some(elem) = elem_opt {
            chart.draw_series(std::iter::once(elem)).unwrap();
            // To avoid the IO failure being ignored silently, we manually call the present function
            root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
            println!("Result has been saved to {fpath}");
        }
        Ok(())
    }
    /// renders this renderable object
    /// # Errors
    /// This method errors if the linspace function fails
    fn render(
        &self,
        view_point: Point3<Length>,
        opening_angle: Point2<Angle>,
        view_target: Point3<Length>,
        view_plane_distance: Length,
        up_direction_opt: Option<Vector3<f64>>,
        (x_pixels, y_pixels): (u32, u32),
    ) -> OpmResult<Vec<f64>> {
        let view_plane_z_dist_m = view_plane_distance.value;
        let ray_origin_m = Point3::new(view_point.x.value, view_point.y.value, view_point.z.value);
        let ray_origin_vec_norm =
            Vector3::new(view_point.x.value, view_point.z.value, view_point.z.value).normalize();
        let light_source_pos = Point3::<f64>::new(50., 100., 50.0);
        let light_source_vec =
            Vector3::new(light_source_pos.x, light_source_pos.y, light_source_pos.z).normalize();
        // get isometry_matrix
        let isometry = up_direction_opt.map_or_else(
            || Isometry::new_from_view_on_target(view_point, view_target, Vector3::y()),
            |up_direction| Isometry::new_from_view_on_target(view_point, view_target, up_direction),
        );

        let x_half_width = (opening_angle.x.get::<radian>() / 2.).tan() * view_plane_z_dist_m;
        let y_half_width = (opening_angle.y.get::<radian>() / 2.).tan() * view_plane_z_dist_m;

        let x_origin = linspace(-x_half_width, x_half_width, x_pixels as usize)?;
        let y_origin = linspace(-y_half_width, y_half_width, y_pixels as usize)?;

        let len_color_vals = (x_pixels * y_pixels * 3) as usize;
        let mut image = vec![1.; len_color_vals];

        image
            .par_chunks_mut((x_pixels) as usize * 3)
            .enumerate()
            .for_each(|(y_id, tile)| {
                for (ix, x) in x_origin.iter().rev().enumerate() {
                    let idx = ix * 3;
                    let ray_start_pre_transform = Point3::new(
                        *x,
                        y_origin[y_pixels as usize - y_id - 1],
                        view_plane_z_dist_m,
                    );
                    let mut ray_start = isometry.transform_point_f64(&ray_start_pre_transform);
                    let ray_dir = Vector3::new(
                        ray_start.x - ray_origin_m.x,
                        ray_start.y - ray_origin_m.y,
                        ray_start.z - ray_origin_m.z,
                    )
                    .normalize();
                    self.render_pixel(
                        &mut ray_start,
                        &ray_dir,
                        &ray_origin_vec_norm,
                        &light_source_vec,
                        &light_source_pos,
                        &mut tile[idx..idx + 3],
                    );
                }
            });

        Ok(image)
    }

    fn render_pixel(
        &self,
        p: &mut Point3<f64>,
        dir: &Vector3<f64>,
        view_source_vec: &Vector3<f64>,
        light_source_vec: &Vector3<f64>,
        light_source_pos: &Point3<f64>,
        color: &mut [f64],
    ) {
        if let Some((p_sdf, c, _)) = self.march_ray(p, dir, MAX_DIST) {
            let normal = self.approx_normal_fast(p, p_sdf);

            if !normal[0].is_finite() || !normal[1].is_finite() || !normal[2].is_finite() {
                color.iter_mut().for_each(|x| *x = 0.);
            }

            color.iter_mut().enumerate().for_each(|(i, x)| *x = c[i]);

            let diffuse_strength = light_source_vec.dot(&normal).max(0.0);

            let reflect_source =
                (-light_source_vec + 2. * normal.dot(light_source_vec) * normal).normalize();
            let specular_strength = reflect_source.dot(view_source_vec).max(0.0).powi(64);

            let dist_to_light_source = (light_source_pos - *p).norm();
            let mut shadow_pos =
                *p + Vector3::new(normal.x * 0.001, normal.y * 0.001, normal.z * 0.001);
            let shadow_dir = (light_source_pos - shadow_pos).normalize();

            if let Some((_, _, dist)) =
                self.march_ray(&mut shadow_pos, &shadow_dir, dist_to_light_source)
            {
                if dist < dist_to_light_source {
                    color.iter_mut().for_each(|x| *x *= 0.25);
                }
            }
            for x in color.iter_mut() {
                *x =
                    (*x * diffuse_strength.mul_add(0.75, specular_strength * 0.25)).powf(1.0 / 2.2);
            }
        } else {
            color.iter_mut().for_each(|x| *x = 0.);
        }
    }

    fn march_ray(
        &self,
        pos: &mut Point3<f64>,
        dir: &Vector3<f64>,
        max_dist: f64,
    ) -> Option<(f64, Vector3<f64>, f64)> {
        let mut dist = 0.;
        for _ in 0_u8..32 {
            let signed_distance = self.sdf_eval_point(pos);

            if signed_distance < MIN_DIST {
                break;
            }
            *pos += signed_distance * dir;
            dist += signed_distance;

            if dist > max_dist {
                return None;
            }
        }
        // let pos =pos + dist*dir ;
        let sdf= self.sdf_eval_point(pos);
        Some((sdf, Vector3::new(0.8,0.8,0.8), dist))
    }

    fn approx_normal(&self, p: &Point3<f64>) -> Vector3<f64> {
        let d = Vector2::new(1e-5, 0.);
        let gx = self.sdf_eval_point(&(p + d.xyy())) - self.sdf_eval_point(&(p - d.xyy()));
        let gy = self.sdf_eval_point(&(p + d.yxy())) - self.sdf_eval_point(&(p - d.yxy()));
        let gz = self.sdf_eval_point(&(p + d.yyx())) - self.sdf_eval_point(&(p - d.yyx()));

        Vector3::new(gx, gy, gz).normalize()
    }

    fn approx_normal_fast(&self, p: &Point3<f64>, p_sdf: f64) -> Vector3<f64> {
        let d = Vector2::new(1e-5, 0.);
        let gx = self.sdf_eval_point(&(p + d.xyy())) - p_sdf;
        let gy = self.sdf_eval_point(&(p + d.yxy())) - p_sdf;
        let gz = self.sdf_eval_point(&(p + d.yyx())) - p_sdf;

        Vector3::new(gx, gy, gz).normalize()
    }

    /// Calculation of a union of signed distance functions for a vector of points.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the objects' union for each input point
    fn sdf_union_vec_of_points(
        &self,
        sdf_vec: &[Surf],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.is_empty() {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_eval_point(p).min(arg0)
                }));
            }
            sdf_out
        }
    }

    /// Calculation of a union of signed distance functions for a single point.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the objects' union for each input point
    fn sdf_union_point(&self, sdf_vec: &[Surf], p: &Point3<f64>) -> f64 {
        if sdf_vec.is_empty() {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_eval_point(p).min(arg0)
            })
        }
    }

    /// Calculation of a union of signed distance functions for a single point.
    /// The union of difference objects is calculated by taking the minimum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the objects' union for each input point
    // fn sdf_union_point_with_color(
    //     &self,
    //     sdf_vec: &[Surf],
    //     p: &Point3<f64>,
    // ) -> (f64, Vector3<f64>) {
    //     if sdf_vec.is_empty() {
    //         self.sdf_eval_with_color(p)
    //     } else {
    //         sdf_vec
    //             .iter()
    //             .fold(self.sdf_eval_with_color(p), |arg0, sdf| {
    //                 let val = sdf.sdf_eval_with_color(p);
    //                 if val.0 < arg0.0 {
    //                     val
    //                 } else {
    //                     arg0
    //                 }
    //             })
    //     }
    // }

    /// Calculation of an intersection of signed distance functions for a vector of points.
    /// The intersection of difference objects is calculated by taking the maximum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the objects' intersection for each input point
    fn sdf_intersection_vec_of_points(
        &self,
        sdf_vec: &[Surf],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.is_empty() {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    sdf.sdf_eval_point(p).max(arg0)
                }));
            }
            sdf_out
        }
    }

    // Calculation of an intersection of signed distance functions for a single point.
    /// The intersection of difference objects is calculated by taking the maximum value of the sdf-values of all objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the objects' intersection for each input point
    fn sdf_intersection_point(&self, sdf_vec: &[Surf], p: &Point3<f64>) -> f64 {
        if sdf_vec.is_empty() {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                sdf.sdf_eval_point(p).max(arg0)
            })
        }
    }

    /// Calculation of a subtraction of signed distance functions for a vector of points.
    /// The subtraction of difference objects is calculated by taking the maximum value of the object to subtract from (self) and the negative value of all other objects.
    /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p_vec`: Vector of 3D points filled with xyz coordinates of type Length
    /// # Returns
    /// Returns a vector of Length with the signed distance of the input object (self), subtracted by all other objects intersection for each input point
    fn sdf_subtraction_vec_of_points(
        &self,
        sdf_vec: &[Surf],
        p_vec: &Vec<Point3<f64>>,
    ) -> Vec<f64> {
        if sdf_vec.is_empty() {
            self.sdf_eval_vec_of_points(p_vec)
        } else {
            let mut sdf_out = Vec::<f64>::with_capacity(p_vec.len());
            for p in p_vec {
                sdf_out.push(sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                    arg0.max(-sdf.sdf_eval_point(p))
                }));
            }
            sdf_out
        }
    }

    // Calculation of a subtraction of signed distance functions for a single point.
    /// The subtraction of difference objects is calculated by taking the maximum value of the object to subtract from (self) and the negative value of all other objects.    
    /// /// # Arguments
    /// - `sdf_vec`: Vector of objects that implement the [`Renderable`] trait
    /// - `p`: 3D point of type Length
    /// # Returns
    /// Returns a Point3 of Length with the signed distance of the input object (self), subtracted by all other objects intersection
    fn sdf_subtraction_point(&self, sdf_vec: &[Surf], p: &Point3<f64>) -> f64 {
        if sdf_vec.is_empty() {
            self.sdf_eval_point(p)
        } else {
            sdf_vec.iter().fold(self.sdf_eval_point(p), |arg0, sdf| {
                arg0.max(-sdf.sdf_eval_point(p))
            })
        }
    }
}
