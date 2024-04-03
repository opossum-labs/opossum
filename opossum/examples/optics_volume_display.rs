use std::collections::HashSet;
use std::time::Instant;

use fast_surface_nets::ndshape::{ConstShape, ConstShape3u32};
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use itertools::Itertools;
use nalgebra::{Matrix3xX, MatrixXx3, Point3, Vector3};
use opossum::{centimeter, meter};
use opossum::plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType};
use opossum::render::{Render, SDFObj, SDFCollection};
use opossum::surface::{Plane, Sphere};
use opossum::utils::griddata::linspace;
use opossum::{degree, error::OpmResult, millimeter, surface::{Cuboid,Cylinder}};
use plotters::style::RGBAColor;
use uom::si::length::millimeter;


// A 16^3 chunk with 1-voxel boundary padding.
// type ChunkShape = ConstShape3u32<60, 60, 60>;
fn main() -> OpmResult<()> {
    const chunk_size: u32 = 4;
    type ChunkShape = ConstShape3u32<chunk_size, chunk_size, chunk_size>;

    let window_size = 0.5;
    let cylinder = Cylinder::new(
        millimeter!(4.),
        millimeter!(1.),
        millimeter!(0., 0., 0.),
        Vector3::z(),
    )?; 
    let cuboid = Cuboid::new(centimeter!(10.,10.,10.), Point3::origin(), Vector3::z())?;
    let sphere1 = Sphere::new(centimeter!(5.), centimeter!(-1.,0.,0.))?;
    let plane = Plane::new(meter!(0.00), Vector3::y(), centimeter!(0.,0.,0.))?;

    let sdf_objs = vec![SDFObj::new(&plane), SDFObj::new(&sphere1)];
    let sdf_objs = vec![SDFObj::new(&plane)];
    let sdf_collection = SDFCollection::new(sdf_objs, None).unwrap();

    let now = Instant::now();
    sdf_collection.render(
        centimeter!(0.,10.,0.), 
        degree!(90.,90.), 
        centimeter!(0.,0.,0.), 
        centimeter!(1.), 
        Some(Vector3::x()),
        (100,100));

    let elapsed_time = now.elapsed();
    println!("Running render() took {} milliseconds.", elapsed_time.as_millis());
    // let sphere2 = Sphere::new(millimeter!(-2.), millimeter!(1.))?;

    // // This chunk will cover just a single octant of a sphere SDF (radius 15).
    // let mut sdf = [1.0; ChunkShape::USIZE];
    // let x_lin = linspace(-window_size/2., window_size/2., chunk_size as f64)?;
    // let y_lin = linspace(-window_size/2., window_size/2., chunk_size as f64)?;
    // let z_lin = linspace(-window_size/2., window_size/2., chunk_size as f64)?;
    // for (ix, x) in x_lin.iter().enumerate() {
    //     for (iy, y) in y_lin.iter().enumerate() {
    //         for (iz, z) in z_lin.iter().enumerate() {
    //             // sdf[iz*60*60 + iy*60 + ix] = cylinder.sdf_intersection_point(vec![&sphere1, &sphere2], &millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
    //             // sdf[iz*60*60 + iy*60 + ix] = cylinder.sdf_union_point(vec![&sphere1, &sphere2], &millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
    //             sdf[iz*(chunk_size*chunk_size)as usize  + iy*chunk_size as usize + ix] = cuboid.sdf_eval_point(&millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
    //             // sdf[iz * 60 * 60 + iy * 60 + ix] = cylinder.sdf_subtraction_point(vec![&sphere1, &sphere2], &millimeter!(*x, *y, *z))
    //             //     .get::<millimeter>() as f32;
    //         }
    //     }
    // }

    // let mut buffer = SurfaceNetsBuffer::default();
    // surface_nets(&sdf, &ChunkShape {}, [0; 3], [chunk_size-1; 3], &mut buffer);

    // let mut plt_params = PlotParameters::default();
    // plt_params
    //     .set(&PlotArgs::FName("cylinder_render_test.png".into()))
    //     .unwrap()
    //     .set(&PlotArgs::FDir("./opossum/playground/".into()))
    //     .unwrap()
    //     .set(&PlotArgs::PlotSize((1000, 1000)))
    //     .unwrap()
    //     .set(&PlotArgs::XLim(AxLims::new(-4., 4.)))
    //     .unwrap()
    //     .set(&PlotArgs::YLim(AxLims::new(-4., 4.)))
    //     .unwrap()
    //     .set(&PlotArgs::ZLim(AxLims::new(-4., 4.)))
    //     .unwrap()
    //     .set(&PlotArgs::ExpandBounds(false))
    //     .unwrap()
    //     .set(&PlotArgs::AxisEqual(false))
    //     .unwrap();

    // let xyz_dat = Matrix3xX::from_vec(
    //     buffer
    //         .positions
    //         .iter()
    //         .flatten()
    //         // .map(|x| ((*x - (chunk_size-1) as f32 / 2.) / (chunk_size-1) as f32) as f64 * window_size)
    //         .map(|x| *x as f64 )
    //         .collect_vec(),
    // )
    // .transpose();
    // let triangle_idx =
    //     Matrix3xX::from_vec(buffer.indices.iter().map(|x| *x as usize).collect_vec()).transpose();
    // let triangle_normals = Matrix3xX::from_vec(buffer.normals.iter().map(|x| [x[0] as f64, x[1] as f64, x[2] as f64] ).flatten().collect_vec()).transpose();
    // let plt_dat = PlotData::TriangulatedSurface {
    //     triangle_idx,
    //     xyz_dat,
    //     triangle_face_normals: triangle_normals
    // };
    // let plt_series = PlotSeries::new(&plt_dat, RGBAColor(200, 200, 200, 1.), None);
    // let plt_type = PlotType::TriangulatedSurface(plt_params);
    // let _ = plt_type.plot(&vec![plt_series]);
    Ok(())
}
