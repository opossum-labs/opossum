use num::Zero;
use opossum::{
    analyzers::{raytrace::AnalysisRayTrace, RayTraceConfig},
    error::OpossumError,
    joule,
    light_result::LightResult,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{EnergyMeter, NodeGroup, Source},
    optic_node::OpticNode,
    properties::Proptype,
    ray::Ray,
    rays::Rays,
    utils::geom_transformation::Isometry,
};
use uom::si::f64::Length;

fn main() {
    println!("Start");
    let mut rays = Rays::default();
    rays.add_ray(
        Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap(),
    );
    rays.add_ray(
        Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.1)).unwrap(),
    );
    let mut scenery = NodeGroup::default();
    let i_s = scenery
        .add_node(&Source::new("src", &LightData::Geometric(rays)))
        .unwrap();
    let mut em = EnergyMeter::default();
    em.set_isometry(Isometry::identity()).unwrap();
    let i_e = scenery.add_node(&em).unwrap();
    scenery
        .connect_nodes(&i_s, "output_1", &i_e, "input_1", Length::zero())
        .unwrap();
    let mut raytrace_config = RayTraceConfig::default();
    raytrace_config.set_min_energy_per_ray(joule!(0.5)).unwrap();
    println!("Before analysis");
    AnalysisRayTrace::analyze(&mut scenery, LightResult::default(), &raytrace_config).unwrap();
    let uuid = scenery
        .node_by_uuid(&i_e)
        .unwrap()
        .uuid()
        .as_simple()
        .to_string();
    let report = scenery
        .node_by_uuid(&i_e)
        .unwrap()
        .optical_ref
        .lock()
        .map_err(|_| OpossumError::Other(format!("Mutex lock failed")))
        .unwrap()
        .node_report(&uuid)
        .unwrap();
    if let Proptype::Energy(e) = report.properties().get("Energy").unwrap() {
        assert_eq!(e, &joule!(1.0));
    } else {
        assert!(false)
    }
}
