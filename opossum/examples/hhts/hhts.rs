use std::path::Path;

mod cambox_1w;
mod cambox_2w;
use cambox_1w::cambox_1w;
use cambox_2w::cambox_2w;

use opossum::{
    error::OpmResult,
    lightdata::LightData,
    nodes::{
        BeamSplitter, EnergyMeter, FilterType, IdealFilter, Lens, Metertype, NodeGroup,
        Propagation, RayPropagationVisualizer, Source, WaveFront,
    },
    position_distributions::Hexapolar,
    ray::SplittingConfig,
    rays::Rays,
    refractive_index::{refr_index_schott::RefrIndexSchott, RefrIndexSellmeier1},
    spectrum::Spectrum,
    spectrum_helper::generate_filter_spectrum,
    OpticScenery,
};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
    length::{millimeter, nanometer},
};

fn main() -> OpmResult<()> {
    let wvl_1w = Length::new::<nanometer>(1054.0);
    let wvl_2w = wvl_1w / 2.0;

    let energy_1w = Energy::new::<joule>(100.0);
    let energy_2w = Energy::new::<joule>(50.0);

    let beam_dist_1w = Hexapolar::new(Length::new::<millimeter>(76.05493), 0)?;
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
    let rays_1w = Rays::new_uniform_collimated(wvl_1w, energy_1w, &beam_dist_1w)?;
    let mut rays_2w = Rays::new_uniform_collimated(wvl_2w, energy_2w, &beam_dist_2w)?;

    // point source

    // use nalgebra::Point3;
    // use num::Zero;
    // use uom::si::{
    //     angle::degree,
    //     f64::Angle,
    // };
    // let rays_1w = Rays::new_hexapolar_point_source(
    //     Point3::new(
    //         Length::zero(),
    //         Length::new::<millimeter>(75.0),
    //         Length::zero(),
    //     ),
    //     Angle::new::<degree>(0.183346572),
    //     6,
    //     wvl_1w,
    //     energy_1w,
    // )?;
    // let mut rays_2w = Rays::new_hexapolar_point_source(
    //     Point3::new(
    //         Length::zero(),
    //         Length::new::<millimeter>(75.0),
    //         Length::zero(),
    //     ),
    //     Angle::new::<degree>(0.183346572),
    //     6,
    //     wvl_2w,
    //     energy_2w,
    // )?;

    let mut rays = rays_1w;
    rays.add_rays(&mut rays_2w);

    let mut scenery = OpticScenery::default();
    scenery.set_description("HHT Sensor")?;

    let src = scenery.add_node(Source::new("Source", &LightData::Geometric(rays)));
    let d1 = scenery.add_node(Propagation::new("d1", Length::new::<millimeter>(2000.0))?);
    scenery.connect_nodes(src, "out1", d1, "front")?;

    // T1
    let mut group_t1 = NodeGroup::new("T1");
    let t1_l1a = group_t1.add_node(Lens::new(
        "T1 L1a",
        Length::new::<millimeter>(518.34008),
        Length::new::<millimeter>(-847.40402),
        Length::new::<millimeter>(30.0),
        &refr_index_hk9l,
    )?)?;
    let d2 = group_t1.add_node(Propagation::new("d2", Length::new::<millimeter>(10.0))?)?;
    let t1_l1b = group_t1.add_node(Lens::new(
        "T1 L1b",
        Length::new::<millimeter>(-788.45031),
        Length::new::<millimeter>(-2551.88619),
        Length::new::<millimeter>(21.66602),
        &refr_index_hzf52,
    )?)?;
    let d3 = group_t1.add_node(Propagation::new(
        "d3",
        Length::new::<millimeter>(937.23608),
    )?)?;
    let t1_l2a = group_t1.add_node(Lens::new(
        "T1 L2a",
        Length::new::<millimeter>(-88.51496),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(5.77736),
        &refr_index_hzf52,
    )?)?;
    let d4 = group_t1.add_node(Propagation::new("d4", Length::new::<millimeter>(8.85423))?)?;
    let t1_l2b = group_t1.add_node(Lens::new(
        "T1 L2b",
        Length::new::<millimeter>(76.76954),
        Length::new::<millimeter>(-118.59590),
        Length::new::<millimeter>(14.0),
        &refr_index_hzf52,
    )?)?;
    let d5 = group_t1.add_node(Propagation::new("d5", Length::new::<millimeter>(14.78269))?)?;
    let t1_l2c = group_t1.add_node(Lens::new(
        "T1 L2c",
        Length::new::<millimeter>(-63.45837),
        Length::new::<millimeter>(66.33014),
        Length::new::<millimeter>(7.68327),
        &refr_index_hzf2,
    )?)?;

    group_t1.connect_nodes(t1_l1a, "rear", d2, "front")?;
    group_t1.connect_nodes(d2, "rear", t1_l1b, "front")?;
    group_t1.connect_nodes(t1_l1b, "rear", d3, "front")?;
    group_t1.connect_nodes(d3, "rear", t1_l2a, "front")?;
    group_t1.connect_nodes(t1_l2a, "rear", d4, "front")?;
    group_t1.connect_nodes(d4, "rear", t1_l2b, "front")?;
    group_t1.connect_nodes(t1_l2b, "rear", d5, "front")?;
    group_t1.connect_nodes(d5, "rear", t1_l2c, "front")?;

    group_t1.map_input_port(t1_l1a, "front", "input")?;
    group_t1.map_output_port(t1_l2c, "rear", "output")?;
    group_t1.expand_view(false)?;
    let t1 = scenery.add_node(group_t1);

    scenery.connect_nodes(d1, "rear", t1, "input")?;

    let d6 = scenery.add_node(Propagation::new("d6", Length::new::<millimeter>(100.0))?);

    scenery.connect_nodes(t1, "output", d6, "front")?;

    // Dichroic beam splitter + filters (1w/2w)

    let mut group_bs = NodeGroup::new("Dichroic beam splitter");

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

    let bs = group_bs.add_node(BeamSplitter::new("Dichroic BS HBSY21", &short_pass)?)?;

    // Long pass filter (1w)
    let felh1000 = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/FELH1000_Transmission.csv",
    )?);
    let filter_1w = group_bs.add_node(IdealFilter::new("1w Longpass filter", &felh1000)?)?;
    group_bs.connect_nodes(bs, "out2_trans2_refl1", filter_1w, "front")?;

    // Long pass filter (2w)
    let fesh0700 = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/FESH0700_Transmission.csv",
    )?);
    let filter_2w = group_bs.add_node(IdealFilter::new("2w Shortpass filter", &fesh0700)?)?;
    group_bs.connect_nodes(bs, "out1_trans1_refl2", filter_2w, "front")?;

    group_bs.map_input_port(bs, "input1", "input")?;
    group_bs.map_output_port(filter_1w, "rear", "output_1w")?;
    group_bs.map_output_port(filter_2w, "rear", "output_2w")?;

    let bs_group = scenery.add_node(group_bs);

    scenery.connect_nodes(d6, "rear", bs_group, "input")?;

    // 1w branch

    // Distance T1 -> T2 1w 637.5190 (-100.0 because of d6)
    let d_1w_7 = scenery.add_node(Propagation::new(
        "1w d7",
        Length::new::<millimeter>(537.5190),
    )?);

    // T2_1w
    let mut group_t2_1w = NodeGroup::new("T2 1w");
    let t2_1w_in = group_t2_1w.add_node(Lens::new(
        "T2 1w In",
        Length::new::<millimeter>(405.38435),
        Length::new::<millimeter>(-702.52114),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_1w_8 = group_t2_1w.add_node(Propagation::new(
        "1w d8",
        Length::new::<millimeter>(442.29480),
    )?)?;
    let t2_1w_field = group_t2_1w.add_node(Lens::new(
        "T2 1w Field",
        Length::new::<millimeter>(179.59020),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_1w_9 = group_t2_1w.add_node(Propagation::new(
        "1w d9",
        Length::new::<millimeter>(429.20520),
    )?)?;
    let t2_1w_exit = group_t2_1w.add_node(Lens::new(
        "T2 1w Exit",
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(-202.81235),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;

    group_t2_1w.connect_nodes(t2_1w_in, "rear", d_1w_8, "front")?;
    group_t2_1w.connect_nodes(d_1w_8, "rear", t2_1w_field, "front")?;
    group_t2_1w.connect_nodes(t2_1w_field, "rear", d_1w_9, "front")?;
    group_t2_1w.connect_nodes(d_1w_9, "rear", t2_1w_exit, "front")?;

    group_t2_1w.map_input_port(t2_1w_in, "front", "input")?;
    group_t2_1w.map_output_port(t2_1w_exit, "rear", "output")?;
    let t2_1w = scenery.add_node(group_t2_1w);

    let d_1w_10 = scenery.add_node(Propagation::new(
        "1w d10",
        Length::new::<millimeter>(664.58900),
    )?);

    // T3_1w
    let mut group_t3_1w = NodeGroup::new("T3 1w");

    let t3_1w_input = group_t3_1w.add_node(Lens::new(
        "T3 1w Input",
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(-417.35031),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_1w_11 = group_t3_1w.add_node(Propagation::new(
        "1w d11",
        Length::new::<millimeter>(1181.0000),
    )?)?;
    let t3_1w_exit = group_t3_1w.add_node(Lens::new(
        "T3 1w Exit",
        Length::new::<millimeter>(156.35054),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_1w_12 = group_t3_1w.add_node(Propagation::new(
        "1w d12",
        Length::new::<millimeter>(279.86873),
    )?)?;
    group_t3_1w.connect_nodes(t3_1w_input, "rear", d_1w_11, "front")?;
    group_t3_1w.connect_nodes(d_1w_11, "rear", t3_1w_exit, "front")?;
    group_t3_1w.connect_nodes(t3_1w_exit, "rear", d_1w_12, "front")?;

    group_t3_1w.map_input_port(t3_1w_input, "front", "input")?;
    group_t3_1w.map_output_port(d_1w_12, "rear", "output")?;
    let t3_1w = scenery.add_node(group_t3_1w);

    scenery.connect_nodes(bs_group, "output_1w", d_1w_7, "front")?;
    scenery.connect_nodes(d_1w_7, "rear", t2_1w, "input")?;
    scenery.connect_nodes(t2_1w, "output", d_1w_10, "front")?;
    scenery.connect_nodes(d_1w_10, "rear", t3_1w, "input")?;

    let mut group_det_1w = NodeGroup::new("Detectors 1w");

    let det_prop = group_det_1w.add_node(RayPropagationVisualizer::new("Propgation"))?;
    let det_wavefront_1w = group_det_1w.add_node(WaveFront::new("Wavefront"))?;
    let cambox_1w = group_det_1w.add_node(cambox_1w()?)?;
    let det_energy_1w =
        group_det_1w.add_node(EnergyMeter::new("Energy", Metertype::IdealEnergyMeter))?;

    group_det_1w.connect_nodes(det_prop, "out1", det_wavefront_1w, "in1")?;
    group_det_1w.connect_nodes(det_wavefront_1w, "out1", det_energy_1w, "in1")?;
    group_det_1w.connect_nodes(det_energy_1w, "out1", cambox_1w, "input")?;

    group_det_1w.map_input_port(det_prop, "in1", "input")?;

    let det_1w = scenery.add_node(group_det_1w);
    scenery.connect_nodes(t3_1w, "output", det_1w, "input")?;

    // 2w branch

    // Distance T1 -> T2 1w 637.5190 (-100.0 because of d6)
    let d_2w_7 = scenery.add_node(Propagation::new(
        "2w d7",
        Length::new::<millimeter>(474.589),
    )?);

    // T2_2w
    let mut group_t2_2w = NodeGroup::new("T2 2w");

    let t2_2w_in = group_t2_2w.add_node(Lens::new(
        "T2 2w In",
        Length::new::<millimeter>(536.5733),
        Length::new::<millimeter>(-677.68238),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_2w_8 = group_t2_2w.add_node(Propagation::new(
        "2w d8",
        Length::new::<millimeter>(409.38829),
    )?)?;
    let t2_2w_field = group_t2_2w.add_node(Lens::new(
        "T2 2w Field",
        Length::new::<millimeter>(208.48421),
        Length::new::<millimeter>(f64::INFINITY),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_2w_9 = group_t2_2w.add_node(Propagation::new(
        "2w d9",
        Length::new::<millimeter>(512.11171),
    )?)?;
    let t2_2w_exit = group_t2_2w.add_node(Lens::new(
        "T2 2w Exit",
        Length::new::<millimeter>(-767.51217),
        Length::new::<millimeter>(-178.98988),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    group_t2_2w.connect_nodes(t2_2w_in, "rear", d_2w_8, "front")?;
    group_t2_2w.connect_nodes(d_2w_8, "rear", t2_2w_field, "front")?;
    group_t2_2w.connect_nodes(t2_2w_field, "rear", d_2w_9, "front")?;
    group_t2_2w.connect_nodes(d_2w_9, "rear", t2_2w_exit, "front")?;

    group_t2_2w.map_input_port(t2_2w_in, "front", "input")?;
    group_t2_2w.map_output_port(t2_2w_exit, "rear", "output")?;
    let t2_2w = scenery.add_node(group_t2_2w);

    let d_2w_10 = scenery.add_node(Propagation::new(
        "2w d10",
        Length::new::<millimeter>(622.09000),
    )?);

    // T3_2w
    let mut group_t3_2w = NodeGroup::new("T3 2w");

    let t3_2w_input = group_t3_2w.add_node(Lens::new(
        "T3 2w Input",
        Length::new::<millimeter>(932.92634),
        Length::new::<millimeter>(-724.14405),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_2w_11 = group_t3_2w.add_node(Propagation::new(
        "2w d11",
        Length::new::<millimeter>(1181.0000),
    )?)?;
    let t3_2w_exit = group_t3_2w.add_node(Lens::new(
        "T3 2w Exit",
        Length::new::<millimeter>(161.31174),
        Length::new::<millimeter>(-1069.52277),
        Length::new::<millimeter>(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_2w_12 = group_t3_2w.add_node(Propagation::new(
        "2w d12",
        Length::new::<millimeter>(250.35850),
    )?)?;
    group_t3_2w.connect_nodes(t3_2w_input, "rear", d_2w_11, "front")?;
    group_t3_2w.connect_nodes(d_2w_11, "rear", t3_2w_exit, "front")?;
    group_t3_2w.connect_nodes(t3_2w_exit, "rear", d_2w_12, "front")?;

    group_t3_2w.map_input_port(t3_2w_input, "front", "input")?;
    group_t3_2w.map_output_port(d_2w_12, "rear", "output")?;
    let t3_2w = scenery.add_node(group_t3_2w);

    scenery.connect_nodes(bs_group, "output_2w", d_2w_7, "front")?;
    scenery.connect_nodes(d_2w_7, "rear", t2_2w, "input")?;
    scenery.connect_nodes(t2_2w, "output", d_2w_10, "front")?;
    scenery.connect_nodes(d_2w_10, "rear", t3_2w, "input")?;

    // 2w detectors
    let mut group_det_2w = NodeGroup::new("Detectors 2w");

    let det_prop_2w = group_det_2w.add_node(RayPropagationVisualizer::new("Propgation"))?;
    let det_wavefront_2w = group_det_2w.add_node(WaveFront::new("Wavefront"))?;
    let det_energy_2w =
        group_det_2w.add_node(EnergyMeter::new("Energy", Metertype::IdealEnergyMeter))?;
    let cambox_2w = group_det_2w.add_node(cambox_2w()?)?;

    group_det_2w.connect_nodes(det_prop_2w, "out1", det_wavefront_2w, "in1")?;
    group_det_2w.connect_nodes(det_wavefront_2w, "out1", det_energy_2w, "in1")?;
    group_det_2w.connect_nodes(det_energy_2w, "out1", cambox_2w, "input")?;

    group_det_2w.map_input_port(det_prop_2w, "in1", "input")?;
    let det_2w = scenery.add_node(group_det_2w);

    scenery.connect_nodes(t3_2w, "output", det_2w, "input")?;
    scenery.save_to_file(Path::new("./opossum/playground/hhts.opm"))?;
    Ok(())
}
