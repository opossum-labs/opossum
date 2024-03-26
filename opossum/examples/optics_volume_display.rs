use fast_surface_nets::ndshape::{ConstShape, ConstShape3u32};
use fast_surface_nets::{surface_nets, SurfaceNetsBuffer};
use itertools::Itertools;
use nalgebra::{Matrix3xX, MatrixXx3, Point3, Vector3};
use opossum::plottable::{AxLims, PlotArgs, PlotData, PlotParameters, PlotSeries, PlotType};
use opossum::signed_distance_function::SDF;
use opossum::surface::Sphere;
use opossum::utils::griddata::linspace;
use opossum::{degree, error::OpmResult, millimeter, surface::cylinder::Cylinder};
use plotters::style::RGBAColor;
use uom::si::length::millimeter;

// A 16^3 chunk with 1-voxel boundary padding.
type ChunkShape = ConstShape3u32<60, 60, 60>;
fn main() -> OpmResult<()> {
    let cylinder = Cylinder::new(
        millimeter!(4.),
        millimeter!(1.),
        millimeter!(0., 0., 0.),
        Vector3::z(),
    )?;
    let sphere1 = Sphere::new(millimeter!(2.), millimeter!(3.))?;
    let sphere2 = Sphere::new(millimeter!(-2.), millimeter!(3.))?;

    // This chunk will cover just a single octant of a sphere SDF (radius 15).
    let mut sdf = [1.0; ChunkShape::USIZE];
    let x_lin = linspace(-3., 3., 60.)?;
    let y_lin = linspace(-3., 3., 60.)?;
    let z_lin = linspace(-3., 3., 60.)?;
    for (ix, x) in x_lin.iter().enumerate() {
        for (iy, y) in y_lin.iter().enumerate() {
            for (iz, z) in z_lin.iter().enumerate() {
                sdf[iz*60*60 + iy*60 + ix] = cylinder.sdf_intersection_point(vec![&sphere1, &sphere2], &millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
                // sdf[iz*60*60 + iy*60 + ix] = cylinder.sdf_union_point(vec![&sphere1, &sphere2], &millimeter!(*x,*y,*z)).get::<millimeter>() as f32;
                // sdf[iz * 60 * 60 + iy * 60 + ix] = cylinder.sdf_subtraction_point(vec![&sphere1, &sphere2], &millimeter!(*x, *y, *z))
                //     .get::<millimeter>() as f32;
            }
        }
    }

    let mut buffer = SurfaceNetsBuffer::default();
    surface_nets(&sdf, &ChunkShape {}, [0; 3], [59; 3], &mut buffer);

    let mut plt_params = PlotParameters::default();
    plt_params
        .set(&PlotArgs::FName("cylinder_render_test.png".into()))
        .unwrap()
        .set(&PlotArgs::FDir("./opossum/playground/".into()))
        .unwrap()
        .set(&PlotArgs::PlotSize((1000, 1000)))
        .unwrap()
        .set(&PlotArgs::XLim(AxLims::new(-3., 3.)))
        .unwrap()
        .set(&PlotArgs::YLim(AxLims::new(-3., 3.)))
        .unwrap()
        .set(&PlotArgs::ZLim(AxLims::new(-3., 3.)))
        .unwrap()
        .set(&PlotArgs::ExpandBounds(false))
        .unwrap()
        .set(&PlotArgs::AxisEqual(false))
        .unwrap();

    let xyz_dat = Matrix3xX::from_vec(
        buffer
            .positions
            .iter()
            .flatten()
            .map(|x| ((*x - 59. / 2.) * 6. / 59.) as f64)
            .collect_vec(),
    )
    .transpose();
    let triangle_idx =
        Matrix3xX::from_vec(buffer.indices.iter().map(|x| *x as usize).collect_vec()).transpose();
    let triangle_normals = Matrix3xX::from_vec(buffer.normals.iter().map(|x| [x[0] as f64, x[1] as f64, x[2] as f64] ).flatten().collect_vec()).transpose();
    let plt_dat = PlotData::TriangulatedSurface {
        triangle_idx,
        xyz_dat,
        triangle_normals
    };
    let plt_series = PlotSeries::new(&plt_dat, RGBAColor(200, 200, 200, 1.), None);
    let plt_type = PlotType::TriangulatedSurface(plt_params);
    let _ = plt_type.plot(&vec![plt_series]);
    Ok(())
}
