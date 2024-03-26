
use itertools::Itertools;
use nalgebra::{Matrix3xX, MatrixXx3, Point3, Vector3};
use opossum::plottable::{PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType};
use opossum::signed_distance_function::SDF;
use opossum::surface::Sphere;
use opossum::utils::griddata::linspace;
use opossum::{
    millimeter, degree,
    error::OpmResult,
    surface::cylinder::Cylinder,
};
use plotters::style::RGBAColor;
use uom::si::length::millimeter;
use fast_surface_nets::ndshape::{ConstShape, ConstShape3u32};
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};

// A 16^3 chunk with 1-voxel boundary padding.
type ChunkShape = ConstShape3u32<50, 50, 50>;
fn main()-> OpmResult<()>{
    let cylinder = Cylinder::new(millimeter!(2.), millimeter!(1.), Point3::origin(), Vector3::z())?;
    let sphere = Sphere::new(millimeter!(0.), millimeter!(1.))?;

    // This chunk will cover just a single octant of a sphere SDF (radius 15).
    let mut sdf = [1.0; ChunkShape::USIZE];
    let x_lin = linspace(-4., 4., 50.)?;
    let y_lin = linspace(-4., 4., 50.)?;
    let z_lin = linspace(-4., 4., 50.)?;
    for (ix, x) in x_lin.iter().enumerate(){
        for (iy, y) in y_lin.iter().enumerate(){
            for (iz, z) in z_lin.iter().enumerate(){
                sdf[iz*50*50 + iy*50 + ix] = sphere.eval_point(&millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
            }
        }
    }

    let mut buffer = SurfaceNetsBuffer::default();
    surface_nets(&sdf, &ChunkShape {}, [0; 3], [49; 3], &mut buffer);

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("cylinder_render_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::PlotSize((1000, 1000)))
        .unwrap();

    let xyz_dat = Matrix3xX::from_vec(buffer.positions.iter().flatten().map(|x| *x as f64).collect_vec()).transpose();
    let triangle_idx = Matrix3xX::from_vec(buffer.indices.iter().map(|x| *x as usize).collect_vec()).transpose();
    let plt_dat = PlotData::TriangulatedSurface { triangle_idx, xyz_dat };
    let plt_series = PlotSeries::new(
        &plt_dat,
        RGBAColor(200, 200, 200, 1.),
        None,
    );
    let plt_type = PlotType::TriangulatedSurface(plt_params);
    let _ = plt_type.plot(&vec![plt_series]);    
    Ok(())
}