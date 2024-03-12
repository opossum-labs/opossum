use std::path::Path;

use nalgebra::Point3;
use num::Zero;
use opossum::{
    error::OpmResult,
    lightdata::LightData,
    nodes::{
        BeamSplitter, EnergyMeter, FilterType, IdealFilter, Lens, Metertype, Propagation,
        RayPropagationVisualizer, Source, SpotDiagram, WaveFront,
    },
    position_distributions::Hexapolar,
    rays::Rays,
    refractive_index::{refr_index_schott::RefrIndexSchott, RefrIndexSellmeier1},
    spectrum::Spectrum,
    spectrum_helper::generate_filter_spectrum,
    OpticScenery, SplittingConfig,
};
use uom::si::{
    angle::degree,
    energy::joule,
    f64::{Angle, Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let wvl_1w = Length::new::<nanometer>(1054.0);
    let wvl_2w = wvl_1w / 2.0;

    let energy_1w = Energy::new::<joule>(100.0);
    let energy_2w = Energy::new::<joule>(50.0);

    let beam_dist_1w = Hexapolar::new(Length::new::<millimeter>(76.05493), 6)?;
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

    // collimated source
    // let rays_1w = Rays::new_uniform_collimated(wvl_1w, energy_1w, &beam_dist_1w)?;
    // let mut rays_2w = Rays::new_uniform_collimated(wvl_2w, energy_2w, &beam_dist_2w)?;

    // point source
    let rays_1w = Rays::new_hexapolar_point_source(
        Point3::new(
            Length::zero(),
            Length::new::<millimeter>(75.0),
            Length::zero(),
        ),
        Angle::new::<degree>(0.183346572),
        6,
        wvl_1w,
        energy_1w,
    )?;
    let mut rays_2w = Rays::new_hexapolar_point_source(
        Point3::new(
            Length::zero(),
            Length::new::<millimeter>(75.0),
            Length::zero(),
        ),
        Angle::new::<degree>(0.183346572),
        6,
        wvl_2w,
        energy_2w,
    )?;

    let mut rays = rays_1w;
    rays.add_rays(&mut rays_2w);

    let mut scenery = OpticScenery::default();
    scenery.set_description("HHT Sensor")?;

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
    let t1_l2b = scenery.add_node(Lens::new(
        "T1 L2b",
        Length::new::<millimeter>(76.76954),
        Length::new::<millimeter>(-118.59590),
        Length::new::<millimeter>(14.0),
        &refr_index_hzf52,
    )?);
    let d5 = scenery.add_node(Propagation::new("d5", Length::new::<millimeter>(14.78269))?);
    let t1_l2c = scenery.add_node(Lens::new(
        "T1 L2c",
        Length::new::<millimeter>(-63.45837),
        Length::new::<millimeter>(66.33014),
        Length::new::<millimeter>(7.68327),
        &refr_index_hzf2,
    )?);
    let d6 = scenery.add_node(Propagation::new("d6", Length::new::<millimeter>(100.0))?);
    scenery.connect_nodes(src, "out1", d1, "front")?;
    scenery.connect_nodes(d1, "rear", t1_l1a, "front")?;
    scenery.connect_nodes(t1_l1a, "rear", d2, "front")?;
    scenery.connect_nodes(d2, "rear", t1_l1b, "front")?;
    scenery.connect_nodes(t1_l1b, "rear", d3, "front")?;
    scenery.connect_nodes(d3, "rear", t1_l2a, "front")?;
    scenery.connect_nodes(t1_l2a, "rear", d4, "front")?;
    scenery.connect_nodes(d4, "rear", t1_l2b, "front")?;
    scenery.connect_nodes(t1_l2b, "rear", d5, "front")?;
    scenery.connect_nodes(d5, "rear", t1_l2c, "front")?;
    scenery.connect_nodes(t1_l2c, "rear", d6, "front")?;

    // Dichroic beam splitter (1w/2w)

    // ideal spectrum
    let short_pass_spectrum = generate_filter_spectrum(
        Length::new::<nanometer>(400.0)..Length::new::<nanometer>(2000.0),
        Length::new::<nanometer>(1.0),
        &opossum::spectrum_helper::FilterType::ShortPassStep {
            cut_off: Length::new::<nanometer>(700.0),
        },
    )?;
    let short_pass = SplittingConfig::Spectrum(short_pass_spectrum);

    // real spectrum (Thorlabs HBSY21)
    //let hbsyx2 = SplittingConfig::Spectrum(Spectrum::from_csv("opossum/examples/hhts/HBSYx2_Reflectivity_45deg_unpol.csv")?);

    let bs = scenery.add_node(BeamSplitter::new("Dichroic BS HBSY21", &short_pass)?);
    scenery.connect_nodes(d6, "rear", bs, "input1")?;

    // 1w branch

    // Long pass filter
    let felh1000 = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/FELH1000_Transmission.csv",
    )?);
    let filter_1w = scenery.add_node(IdealFilter::new("1w Longpass filter", &felh1000)?);
    scenery.connect_nodes(bs, "out2_trans2_refl1", filter_1w, "front")?;

    // Distance T1 -> T2 1w 637.5190 (-100.0 because of d6)
    let d_1w_7 = scenery.add_node(Propagation::new(
        "1w d7",
        Length::new::<millimeter>(537.5190),
    )?);
    let t2_1w_in = scenery.add_node(Lens::new(
        "T2 1w In",
        Length::new::<millimeter>(405.38435),
        Length::new::<millimeter>(-702.52114),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_1w_8 = scenery.add_node(Propagation::new(
        "1w d8",
        Length::new::<millimeter>(442.29480),
    )?);
    let t2_1w_field = scenery.add_node(Lens::new(
        "T2 1w Field",
        Length::new::<millimeter>(179.59020),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_1w_9 = scenery.add_node(Propagation::new(
        "1w d9",
        Length::new::<millimeter>(429.20520),
    )?);
    let t2_1w_exit = scenery.add_node(Lens::new(
        "T2 1w Exit",
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(-202.81235),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_1w_10 = scenery.add_node(Propagation::new(
        "1w d10",
        Length::new::<millimeter>(664.58900),
    )?);
    let t3_1w_input = scenery.add_node(Lens::new(
        "T3 1w Input",
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(-417.35031),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_1w_11 = scenery.add_node(Propagation::new(
        "1w d11",
        Length::new::<millimeter>(1181.0000),
    )?);
    let t3_1w_exit = scenery.add_node(Lens::new(
        "T3 1w Exit",
        Length::new::<millimeter>(156.35054),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_1w_12 = scenery.add_node(Propagation::new(
        "1w d12",
        Length::new::<millimeter>(279.86873),
    )?);

    scenery.connect_nodes(filter_1w, "rear", d_1w_7, "front")?;
    scenery.connect_nodes(d_1w_7, "rear", t2_1w_in, "front")?;
    scenery.connect_nodes(t2_1w_in, "rear", d_1w_8, "front")?;
    scenery.connect_nodes(d_1w_8, "rear", t2_1w_field, "front")?;
    scenery.connect_nodes(t2_1w_field, "rear", d_1w_9, "front")?;
    scenery.connect_nodes(d_1w_9, "rear", t2_1w_exit, "front")?;
    scenery.connect_nodes(t2_1w_exit, "rear", d_1w_10, "front")?;
    scenery.connect_nodes(d_1w_10, "rear", t3_1w_input, "front")?;
    scenery.connect_nodes(t3_1w_input, "rear", d_1w_11, "front")?;
    scenery.connect_nodes(d_1w_11, "rear", t3_1w_exit, "front")?;
    scenery.connect_nodes(t3_1w_exit, "rear", d_1w_12, "front")?;

    let det_prop = scenery.add_node(RayPropagationVisualizer::new("Ray propgation 1w"));
    scenery.connect_nodes(d_1w_12, "rear", det_prop, "in1")?;

    let det_wavefront_1w = scenery.add_node(WaveFront::new("Wavefront 1w"));
    scenery.connect_nodes(det_prop, "out1", det_wavefront_1w, "in1")?;

    let det_spot_diagram_1w = scenery.add_node(SpotDiagram::new("Spot diagram 1w"));
    scenery.connect_nodes(det_wavefront_1w, "out1", det_spot_diagram_1w, "in1")?;

    let det_energy_1w =
        scenery.add_node(EnergyMeter::new("Energy 1w", Metertype::IdealEnergyMeter));
    scenery.connect_nodes(det_spot_diagram_1w, "out1", det_energy_1w, "in1")?;

    // 2w branch

    // Long pass filter
    let fesh0700 = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/FESH0700_Transmission.csv",
    )?);
    let filter_2w = scenery.add_node(IdealFilter::new("2w Shortpass filter", &fesh0700)?);
    scenery.connect_nodes(bs, "out1_trans1_refl2", filter_2w, "front")?;

    // Distance T1 -> T2 1w 637.5190 (-100.0 because of d6)
    let d_2w_7 = scenery.add_node(Propagation::new(
        "2w d7",
        Length::new::<millimeter>(474.589),
    )?);
    let t2_2w_in = scenery.add_node(Lens::new(
        "T2 2w In",
        Length::new::<millimeter>(536.5733),
        Length::new::<millimeter>(-677.68238),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_2w_8 = scenery.add_node(Propagation::new(
        "2w d8",
        Length::new::<millimeter>(409.38829),
    )?);
    let t2_2w_field = scenery.add_node(Lens::new(
        "T2 2w Field",
        Length::new::<millimeter>(208.48421),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_2w_9 = scenery.add_node(Propagation::new(
        "2w d9",
        Length::new::<millimeter>(512.11171),
    )?);
    let t2_2w_exit = scenery.add_node(Lens::new(
        "T2 2w Exit",
        Length::new::<millimeter>(-767.51217),
        Length::new::<millimeter>(-178.98988),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_2w_10 = scenery.add_node(Propagation::new(
        "2w d10",
        Length::new::<millimeter>(622.09000),
    )?);
    let t3_2w_input = scenery.add_node(Lens::new(
        "T3 2w Input",
        Length::new::<millimeter>(932.92634),
        Length::new::<millimeter>(-724.14405),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_2w_11 = scenery.add_node(Propagation::new(
        "2w d11",
        Length::new::<millimeter>(1181.0000),
    )?);
    let t3_2w_exit = scenery.add_node(Lens::new(
        "T3 2w Exit",
        Length::new::<millimeter>(161.31174),
        Length::new::<millimeter>(-1069.52277),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?);
    let d_2w_12 = scenery.add_node(Propagation::new(
        "2w d12",
        Length::new::<millimeter>(250.35850),
    )?);

    scenery.connect_nodes(filter_2w, "rear", d_2w_7, "front")?;
    scenery.connect_nodes(d_2w_7, "rear", t2_2w_in, "front")?;
    scenery.connect_nodes(t2_2w_in, "rear", d_2w_8, "front")?;
    scenery.connect_nodes(d_2w_8, "rear", t2_2w_field, "front")?;
    scenery.connect_nodes(t2_2w_field, "rear", d_2w_9, "front")?;
    scenery.connect_nodes(d_2w_9, "rear", t2_2w_exit, "front")?;
    scenery.connect_nodes(t2_2w_exit, "rear", d_2w_10, "front")?;
    scenery.connect_nodes(d_2w_10, "rear", t3_2w_input, "front")?;
    scenery.connect_nodes(t3_2w_input, "rear", d_2w_11, "front")?;
    scenery.connect_nodes(d_2w_11, "rear", t3_2w_exit, "front")?;
    scenery.connect_nodes(t3_2w_exit, "rear", d_2w_12, "front")?;

    let det_prop_2w = scenery.add_node(RayPropagationVisualizer::new("Ray propgation 2w"));
    scenery.connect_nodes(d_2w_12, "rear", det_prop_2w, "in1")?;

    let det_wavefront_2w = scenery.add_node(WaveFront::new("Wavefront 2w"));
    scenery.connect_nodes(det_prop_2w, "out1", det_wavefront_2w, "in1")?;

    let det_spot_diagram_2w = scenery.add_node(SpotDiagram::new("Spot diagram 2w"));
    scenery.connect_nodes(det_wavefront_2w, "out1", det_spot_diagram_2w, "in1")?;

    let det_energy_2w =
        scenery.add_node(EnergyMeter::new("Energy 2w", Metertype::IdealEnergyMeter));
    scenery.connect_nodes(det_spot_diagram_2w, "out1", det_energy_2w, "in1")?;

    scenery.save_to_file(Path::new("./opossum/playground/hhts.opm"))?;
    Ok(())
}
