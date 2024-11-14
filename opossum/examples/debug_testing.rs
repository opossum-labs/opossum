use nalgebra::vector;
use opossum::{
    analyzers::{raytrace::AnalysisRayTrace, RayTraceConfig},
    degree, joule,
    light_result::LightResult,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::ThinMirror,
    optic_node::OpticNode,
    ray::Ray,
    rays::Rays,
    utils::geom_transformation::Isometry,
};

fn main() {
    let mut node = ThinMirror::default();

    let _ = node
        .set_isometry(Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(0.0, 0.0, 0.0)).unwrap());
    let mut input = LightResult::default();
    let mut rays = Rays::default();
    rays.add_ray(Ray::origin_along_z(nanometer!(1000.0), joule!(1.0)).unwrap());
    let input_light = LightData::Geometric(rays);
    input.insert("input_1".into(), input_light.clone());
    let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
    if let Some(LightData::Geometric(rays)) = output.get("output_1") {
        assert_eq!(rays.nr_of_rays(false), 1);
        let ray = rays.iter().next().unwrap();
        assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
        let dir = vector![0.0, 0.0, -1.0];
        assert_eq!(ray.direction(), dir);
    } else {
        assert!(false, "could not get LightData");
    }
}
