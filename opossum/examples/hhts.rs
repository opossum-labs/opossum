use opossum::{
    error::OpmResult, lightdata::LightData, nodes::Source, position_distributions::Hexapolar, rays::Rays, refractive_index::{refr_index_schott::RefrIndexSchott, RefrIndexSellmeier1}, OpticScenery
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
    scenery.set_description("HHT Sensor");

    let src=scenery.add_node(Source::new("src", &LightData::Geometric(rays)));

    
    Ok(())
}
