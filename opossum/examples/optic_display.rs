use nalgebra::{Point3, Vector3};
use opossum::{
    centimeter, degree,
    error::OpmResult,
    joule, millimeter,
    nodes::{collimated_line_ray_source, Lens, RayPropagationVisualizer, SpotDiagram, Wedge},
    optical::Alignable,
    refractive_index::RefrIndexConst,
    render::{SDFCollection, SDFOperation},
    surface::{Cylinder, Sphere},
    OpticScenery,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let r1 = 2000.;
    let r2 = 2000.;
    let thickness = 8.;
    let diameter = 25.;
    let cylinder = Cylinder::new(
        millimeter!(100.),
        millimeter!(diameter / 2.),
        millimeter!(0., 0., 0.),
        Vector3::z(),
    )?;
    let sphere1 =
        Sphere::new_from_position(centimeter!(r1), centimeter!(-r1 + thickness / 20., 0., 0.))?;
    let sphere2 =
        Sphere::new_from_position(centimeter!(r2), centimeter!(r2 - thickness / 20., 0., 0.))?;

    // let sdf_collection = SDFCollection::new(
    //     vec![&cylinder, &sphere1, &sphere2],
    //     Some(SDFOperation::Intersection),
    //     tessellation::BoundingBox {
    //         min: Point3::new(-diameter/2., -diameter/2., -(r1*r1 - diameter*diameter).sqrt()),
    //         max: Point3::new(diameter/2., diameter/2., thickness+(r1*r1 - diameter*diameter).sqrt()) }
    // );

    Ok(())
}
