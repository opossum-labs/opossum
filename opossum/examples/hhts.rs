use std::path::Path;

use opossum::{
    error::OpmResult,
    lightdata::LightData,
    nodes::{Lens, Propagation, RayPropagationVisualizer, Source},
    position_distributions::Hexapolar,
    rays::Rays,
    refractive_index::{refr_index_schott::RefrIndexSchott, RefrIndexSellmeier1},
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let wvl_1w = Length::new::<nanometer>(1053.0);
    let wvl_2w = wvl_1w / 2.0;

    let energy_1w = Energy::new::<joule>(100.0);
    let energy_2w = Energy::new::<joule>(100.0);

    let beam_dist_1w = Hexapolar::new(Length::new::<millimeter>(76.05493), 5)?;
    let beam_dist_2w = beam_dist_1w.clone();

    let refr_index_hk9l = RefrIndexSellmeier1::new(
        6.14555251E-1,
        6.56775017E-1,
        1.02699346E+0,
        1.45987884E-2,
        2.87769588E-3,
        1.07653051E+2,
    )?;
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
    )?;
    let refr_index_hzf2 = RefrIndexSellmeier1::new(
        1.67643380E-001,
        1.54335076E+000,
        1.17313123E+000,
        6.05177711E-002,
        1.18524273E-002,
        1.13671100E+002,
    )?;
    let rays_1w = Rays::new_uniform_collimated(wvl_1w, energy_1w, &beam_dist_1w)?;
    let mut rays_2w = Rays::new_uniform_collimated(wvl_2w, energy_2w, &beam_dist_2w)?;

    let mut rays = rays_1w;
    rays.add_rays(&mut rays_2w);

    let mut scenery = OpticScenery::default();
    scenery.set_description("HHT Sensor")?;

    // scenery.add_node(Dummy::default());
    // scenery.add_node(WaveFront::default());
    // scenery.add_node(SpotDiagram::default());
    // scenery.add_node(Spectrometer::default());
    // scenery.add_node(RayPropagationVisualizer::default());
    // scenery.add_node(ParaxialSurface::default());
    // scenery.add_node(Detector::default());
    // scenery.add_node(NodeGroup::default());
    // scenery.add_node(FluenceDetector::default());
    // scenery.add_node(EnergyMeter::default());
    // scenery.add_node(BeamSplitter::default());
    // scenery.add_node(IdealFilter::default());
    // scenery.add_node(Propagation::default());
    // scenery.add_node(Lens::default());

    let src = scenery.add_node(Source::new("src", &LightData::Geometric(rays)));
    let d1 = scenery.add_node(Propagation::new("d1", Length::new::<millimeter>(2000.0))?);
    let t1_l1a = scenery.add_node(Lens::new(
        "T1 L1a",
        Length::new::<millimeter>(518.34008),
        Length::new::<millimeter>(-847.40402),
        Length::new::<millimeter>(30.0),
        &refr_index_hk9l,
    )?);
    let d2 = scenery.add_node(Propagation::new("d2", Length::new::<millimeter>(10.0))?);
    let t1_l1b = scenery.add_node(Lens::new(
        "T1 L1b",
        Length::new::<millimeter>(-788.45031),
        Length::new::<millimeter>(-2551.88619),
        Length::new::<millimeter>(21.66602),
        &refr_index_hzf52,
    )?);
    let d3 = scenery.add_node(Propagation::new(
        "d3",
        Length::new::<millimeter>(937.23608),
    )?);
    let t1_l2a = scenery.add_node(Lens::new(
        "T1 L2a",
        Length::new::<millimeter>(-88.51496),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(5.77736),
        &refr_index_hzf52,
    )?);
    let d4 = scenery.add_node(Propagation::new("d4", Length::new::<millimeter>(8.85423))?);
    scenery.connect_nodes(src, "out1", d1, "front")?;
    scenery.connect_nodes(d1, "rear", t1_l1a, "front")?;
    scenery.connect_nodes(t1_l1a, "rear", d2, "front")?;
    scenery.connect_nodes(d2, "rear", t1_l1b, "front")?;
    scenery.connect_nodes(t1_l1b, "rear", d3, "front")?;
    scenery.connect_nodes(d3, "rear", t1_l2a, "front")?;
    scenery.connect_nodes(t1_l2a, "rear", d4, "front")?;

    let det_prop = scenery.add_node(RayPropagationVisualizer::new("det"));
    scenery.connect_nodes(d4, "rear", det_prop, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/hhts.opm"))?;
    Ok(())
}
