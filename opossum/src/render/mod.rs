mod sdf;
pub use sdf::{SDF, SDFObj, SDFCollection};

use std::time::Instant;
use itertools::Itertools;
use nalgebra::{Point2, Point3, Vector2, Vector3};
use num::ToPrimitive;
use plotters::{backend::BitMapBackend, chart::ChartBuilder, drawing::IntoDrawingArea, element::BitMapElement, style:: WHITE};
use uom::si::{angle::radian, f64::{Angle, Length}};
use crate::{error::{OpmResult, OpossumError}, utils::{geom_transformation::Isometry, griddata::linspace}};

const MIN_DIST: f64 = 10e-6;
const MAX_DIST: f64 = 1e2;

pub trait Render: SDF
{
    fn plot_image(&self, image_dat: Vec<f64>, image_shape: (u32, u32)) -> OpmResult<()>{
        if image_shape.0*image_shape.1*3 != image_dat.len() as u32{
            return Err(OpossumError::Other("Shape of image and vector length does not match! Cannot render image!".into()))
        }
        let fname = "./opossum/playground/.render_test.png";
        let root = BitMapBackend::new(fname, image_shape).into_drawing_area();
        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(&root)
            .build_cartesian_2d(0.0..1.0, 0.0..1.0).unwrap();

        chart.configure_mesh().disable_mesh().disable_y_axis().disable_x_axis().draw().unwrap();
        let u8_image_dat = image_dat.iter().map(|x| (x.abs()*255.).to_u8().unwrap()).collect_vec();

        let elem_opt = BitMapElement::with_owned_buffer((0.,1.0).into(), image_shape, u8_image_dat);

        if let Some(elem) = elem_opt{
            chart.draw_series(std::iter::once(elem)).unwrap();
        // To avoid the IO failure being ignored silently, we manually call the present function
            root.present().expect("Unable to write result to file, please make sure 'plotters-doc-data' dir exists under current dir");
            println!("Result has been saved to {}", fname);
        }
        Ok(())
    }
    fn render(
        &self, 
        view_point: Point3<Length>,
        opening_angle: Point2<Angle>,
        view_target: Point3<Length>,
        view_plane_distance: Length,
        up_direction_opt: Option<Vector3<f64>>,
        (x_pixels, y_pixels): (u32, u32)
    ){  
        let view_plane_z_dist_m = view_plane_distance.value;
        let ray_origin_m = Point3::new(view_point.x.value, view_point.y.value, view_point.z.value);
        let ray_origin_vec_norm = Vector3::new(view_point.x.value, view_point.z.value, view_point.z.value).normalize();
        let light_source = Vector3::<f64>::new(0., 10., 10.0).normalize();
        // get isometry_matrix
        let isometry = if let Some(up_direction) = up_direction_opt{
            Isometry::new_from_view_on_target(view_point, view_target, up_direction)
        }
        else{
            Isometry::new_from_view_on_target(view_point, view_target, Vector3::y())
        };

        let x_half_width = (opening_angle.x.get::<radian>()/2.).tan()*view_plane_z_dist_m;
        let y_half_width = (opening_angle.y.get::<radian>()/2.).tan()*view_plane_z_dist_m;

        let x_origin = linspace(-x_half_width, x_half_width, x_pixels as f64).unwrap();
        let y_origin = linspace(-y_half_width, y_half_width, y_pixels as f64).unwrap();

        let len_color_vals = (x_pixels*y_pixels*3) as usize;
        let mut image = vec![1.; len_color_vals];
        // let mut image = Vec::<f64>::with_capacity((x_pixels*y_pixels*3) as usize);

        for (iy, y) in y_origin.iter().enumerate(){
            let start_idx_row = iy*x_pixels as usize;
            for (ix,x) in x_origin.iter().enumerate(){
                let idx_i = len_color_vals - ( start_idx_row + ix +1)*3;
                let idx_f = idx_i+3;
                let ray_start_pre_transform = Point3::new(*x,*y,view_plane_z_dist_m);
                let ray_start = isometry.transform_point_f64(&ray_start_pre_transform);
                let ray_dir = Vector3::new(ray_start.x-ray_origin_m.x, ray_start.y-ray_origin_m.y, ray_start.z-ray_origin_m.z).normalize();
                self.render_pixel(
                    ray_start, 
                    ray_dir, 
                    ray_origin_m,  
                    &ray_origin_vec_norm, 
                    &light_source, 
                    &mut image[idx_i..idx_f]
                );
            }
        }
        let now = Instant::now();

        self.plot_image(image, (x_pixels, y_pixels));
        let elapsed_time = now.elapsed();
        println!("Creating plot took {} milliseconds.", elapsed_time.as_millis());
    }

    fn render_pixel(
        &self, 
        pos: Point3<f64>, 
        dir: Vector3<f64>, 
        view_origin :Point3<f64>, 
        view_source_vec: &Vector3<f64>, 
        light_source_vec: &Vector3<f64>, 
        color: &mut [f64]
    ){
        if let Some(p) = self.march_ray(pos, dir){
            let normal = self.approx_normal(&p);
            if normal[0].is_infinite() ||  normal[1].is_infinite()  || normal[2].is_infinite() {
                color.iter_mut().for_each(|x| *x = 0.);
            }

            // let light_color = Vector3::<f64>::new(1.0,1.0,1.0);
            // let light_source = Vector3::<f64>::new(2.5, 2.5, 10.0);
            // let light_source_norm = light_source.normalize();
            let diffuse_strength = light_source_vec.dot(&normal).max(0.0);
            // let diffuse = light_color * diffuse_strength;

            // let view_source = Vector3::<f64>::new(view_origin.x, view_origin.y, view_origin.z).normalize();
            let reflect_source = (-light_source_vec + 2.*normal.dot(&light_source_vec)*normal).normalize();
            let specular_strength = reflect_source.dot(&view_source_vec).max(0.0).powi(64);
            // let specular = specular_strength * light_color;

            color.iter_mut().for_each(|x| *x = (*x*(diffuse_strength * 0.75 + specular_strength * 0.25)).powf(1.0 / 2.2));

            // let light_direction = light_source.normalize();
            // let dist_to_light_source = millimeter!((light_source_norm - Vector3::new(p.x.value, p.y.value, p.z.value)).norm());
            // let shadow_pos = p + Vector3::new(millimeter!(normal.x), millimeter!(normal.y), millimeter!(normal.z));
            // let shadow_dir = light_direction;

            // // part 3.2 - ray march based on new ro + rd
            // if let Some(p) = self.march_ray(shadow_pos, shadow_dir, dist_to_light_source){
            //     color *= 0.25;
            // }
            
            // note: add gamma correction
            // color.iter_mut().for_each(|x| *x = x.abs().powf(1.0 / 2.2));

            // (color[0], color[1], color[2])
        }
        else{
            // (0.,0.,0.)
            color.iter_mut().for_each(|x| *x = 0.);
        }
    }

    fn march_ray(&self, pos: Point3<f64>, dir: Vector3<f64>) -> Option<Point3<f64>>{
        let mut dist = 0.;
        for i in 0_u8..32{
            let p = pos + dist*dir;
            let signed_distance = self.sdf_eval_point(&p);

            if signed_distance < MIN_DIST {
                return Some(p);
            }
            dist = dist + signed_distance;

            if dist > MAX_DIST {
                return None;
            }
        }
        Some(pos + dist*dir)
    }

    fn approx_normal(&self, p: &Point3<f64>) -> Vector3<f64>{
        let d = Vector2::new(1e-5, 0.);
        let gx = self.sdf_eval_point(&(p + d.xyy())) - self.sdf_eval_point(&(p - d.xyy()));
        let gy = self.sdf_eval_point(&(p + d.yxy())) - self.sdf_eval_point(&(p - d.yxy()));
        let gz = self.sdf_eval_point(&(p + d.yyx())) - self.sdf_eval_point(&(p - d.yyx()));

        Vector3::new(gx, gy, gz).normalize()
        // Vector3::new(gx.value.abs(), gy.value.abs(), gz.value.abs())/gx.value.abs().max(gy.value.abs()).max(gz.value.abs())
    }
        
}