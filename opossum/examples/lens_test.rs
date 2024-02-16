use opossum::error::OpmResult;
use opossum::nodes::{create_collimated_ray_source, Lens, Propagation, RayPropagationVisualizer};
use opossum::OpticScenery;
use std::path::Path;
use uom::si::energy::joule;
use uom::si::f64::{Energy, Length};
use uom::si::length::millimeter;

fn main() -> OpmResult<()> {
    let mut scenery = OpticScenery::new();
    scenery.set_description("Lens Ray-trace test".into())?;

    let src = scenery.add_node(
        create_collimated_ray_source(
            Length::new::<millimeter>(20.0),
            Energy::new::<joule>(1.0),
            3,
        )
        .unwrap(),
    );
    let s1 = scenery.add_node(Propagation::new("s1", Length::new::<millimeter>(100.0))?);
    let l1 = scenery.add_node(Lens::new(
        Length::new::<millimeter>(50.0),
        Length::new::<millimeter>(-50.0),
        Length::new::<millimeter>(20.0),
        1.5,
    ));
    let s2 = scenery.add_node(Propagation::new("s2", Length::new::<millimeter>(200.0))?);
    let det = scenery.add_node(RayPropagationVisualizer::default());

    scenery.connect_nodes(src, "out1", s1, "front")?;
    scenery.connect_nodes(s1, "rear", l1, "front")?;
    scenery.connect_nodes(l1, "rear", s2, "front")?;
    scenery.connect_nodes(s2, "rear", det, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/lens_system.opm"))?;
    Ok(())
}
