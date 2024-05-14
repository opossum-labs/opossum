use opossum::error::OpmResult;

fn main() -> OpmResult<()> {
    // let cylinder = Cylinder::new(
    //     millimeter!(10.),
    //     millimeter!(12.5),
    //     millimeter!(0., 20., 0.),
    //     Vector3::x(),
    // )?;
    // let _cuboid = Cuboid::new(centimeter!(10., 10., 10.), Point3::origin(), Vector3::z())?;
    // let sphere1 = Sphere::new_from_position(centimeter!(200.), centimeter!(-200. + 0.4, 2., 0.))?;
    // let sphere2 = Sphere::new_from_position(centimeter!(200.), centimeter!(200. - 0.4, 2., 0.))?;
    // let _sphere3 = Sphere::new_from_position(centimeter!(1.), centimeter!(0., 2., 0.))?;

    // let iso = Isometry::new(Point3::origin(), degree!(0.0, 0.0, 0.0))?;
    // let _plane = Plane::new(&iso);
    // let optical_table = OpticalTable::new(
    //     Vector3::new(0.5, 0.5, 0.).normalize(),
    //     centimeter!(0., 0., 2.),
    //     centimeter!(2.5),
    // )?;

    // let sdf_collection = SDFCollection::new(
    //     vec![&cylinder, &sphere1, &sphere2],
    //     Some(SDFOperation::Intersection),
    // )
    // .unwrap();
    // let sdf_collection2 = SDFCollection::new(
    //     vec![&optical_table, &sdf_collection],
    //     Some(SDFOperation::Union),
    // )
    // .unwrap();

    // let now = Instant::now();
    // let image = sdf_collection2
    //     .render(
    //         centimeter!(10., 10., 10.),
    //         degree!(25., 25.),
    //         centimeter!(0., 2., 0.),
    //         centimeter!(1.),
    //         Some(Vector3::y()),
    //         (256, 256),
    //     )
    //     .unwrap();

    // let fpath = "./opossum/playground/.render_test.png";
    // sdf_collection2
    //     .plot_image(&image, (256, 256), fpath)
    //     .unwrap();

    // let elapsed_time = now.elapsed();
    // println!(
    //     "Running render() took {} milliseconds.",
    //     elapsed_time.as_millis()
    // );

    Ok(())
}
