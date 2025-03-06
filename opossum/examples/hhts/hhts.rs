use std::path::Path;

mod cambox_1w;
mod cambox_2w;
mod hhts_input;
use cambox_1w::cambox_1w;
use cambox_2w::cambox_2w;
use hhts_input::hhts_input;

use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
    aperture::{Aperture, CircleConfig},
    energy_distributions::General2DGaussian,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{
        BeamSplitter, Dummy, EnergyMeter, FilterType, IdealFilter, Lens, Metertype, NodeGroup,
        RayPropagationVisualizer, Source, WaveFront,
    },
    optic_node::OpticNode,
    optic_ports::PortType,
    position_distributions::HexagonalTiling,
    radian,
    ray::SplittingConfig,
    rays::Rays,
    refractive_index::{refr_index_schott::RefrIndexSchott, RefrIndexSellmeier1},
    spectrum::Spectrum,
    spectrum_helper::generate_filter_spectrum,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let wvl_1w = nanometer!(1054.0);
    let wvl_2w = wvl_1w / 2.0;

    let energy_1w = joule!(100.0);
    let energy_2w = joule!(50.0);

    // let beam_dist_1w = Hexapolar::new(millimeter!(76.05493), 10)?;
    let beam_dist_1w = HexagonalTiling::new(millimeter!(100.), 10, millimeter!(0., 0.))?;
    let beam_dist_2w = HexagonalTiling::new(millimeter!(100.), 10, millimeter!(1., 1.))?;
    // let beam_dist_2w = beam_dist_1w.clone();

    let refr_index_hk9l = RefrIndexSellmeier1::new(
        6.14555251E-1,
        6.56775017E-1,
        1.02699346E+0,
        1.45987884E-2,
        2.87769588E-3,
        1.07653051E+2,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let refr_index_hzf52 = RefrIndexSchott::new(
        3.26760058E+000,
        -2.05384566E-002,
        3.51507672E-002,
        7.70151348E-003,
        -9.08139817E-004,
        7.52649555E-005,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;
    let refr_index_hzf2 = RefrIndexSellmeier1::new(
        1.67643380E-001,
        1.54335076E+000,
        1.17313123E+000,
        6.05177711E-002,
        1.18524273E-002,
        1.13671100E+002,
        nanometer!(300.0)..nanometer!(2000.0),
    )?;

    // apertures
    // let circle_config = CircleConfig::new(millimeter!(25.4), millimeter!(0., 0.))?;
    // let a_2inch = Aperture::BinaryCircle(circle_config);
    let circle_config = CircleConfig::new(millimeter!(12.7), millimeter!(0., 0.))?;
    let a_1inch = Aperture::BinaryCircle(circle_config);

    // collimated source

    let rays_1w = Rays::new_collimated(
        wvl_1w,
        &General2DGaussian::new(
            energy_1w,
            millimeter!(0., 0.),
            millimeter!(60.6389113608, 60.6389113608),
            5.,
            radian!(0.),
            false,
        )?,
        &beam_dist_1w,
    )?;
    let mut rays_2w = Rays::new_collimated(
        wvl_2w,
        &General2DGaussian::new(
            energy_2w,
            millimeter!(0., 0.),
            millimeter!(60.6389113608, 60.6389113608),
            5.,
            radian!(0.),
            false,
        )?,
        &beam_dist_2w,
    )?;
    // let rays_1w = Rays::new_uniform_collimated(wvl_1w, energy_1w, &beam_dist_1w)?;
    // let mut rays_2w = Rays::new_uniform_collimated(wvl_2w, energy_2w, &beam_dist_2w)?;

    // point source

    // let rays_1w = Rays::new_hexapolar_point_source(
    //     millimeter!(
    //         0.,
    //         75.0,
    //         0.,
    //     ),
    //     degree!(0.183346572),
    //     6,
    //     wvl_1w,
    //     energy_1w,
    // )?;
    // let mut rays_2w = Rays::new_hexapolar_point_source(
    //     millimeter!(
    //         0.,
    //         75.0,
    //         0.,
    //     ),
    //     degree!(0.183346572),
    //     6,
    //     wvl_2w,
    //     energy_2w,
    // )?;

    let mut rays = rays_1w;
    rays.add_rays(&mut rays_2w);

    let mut scenery = NodeGroup::new("HHT Sensor");
    let mut src = Source::new("Source", &LightData::Geometric(rays));
    src.set_isometry(Isometry::identity())?;
    let src = scenery.add_node(src)?;
    let input_group = scenery.add_node(hhts_input()?)?;
    scenery.connect_nodes(&src, "output_1", &input_group, "input_1", Length::zero())?;

    // T1
    let mut group_t1 = NodeGroup::new("T1");
    let t1_l1a = group_t1.add_node(Lens::new(
        "T1 L1a",
        millimeter!(518.34008),
        millimeter!(-847.40402),
        millimeter!(30.0),
        &refr_index_hk9l,
    )?)?;
    let t1_l1b = group_t1.add_node(Lens::new(
        "T1 L1b",
        millimeter!(-788.45031),
        millimeter!(-2551.88619),
        millimeter!(21.66602),
        &refr_index_hzf52,
    )?)?;
    let node = Lens::new(
        "T1 L2a",
        millimeter!(-88.51496),
        millimeter!(f64::INFINITY),
        millimeter!(5.77736),
        &refr_index_hzf52,
    )?;
    // node.set_aperture(&PortType::Input, "input_1", &a_2inch)?;
    // node.set_aperture(&PortType::Output, "output_1", &a_2inch)?;
    let t1_l2a = group_t1.add_node(node)?;
    let t1_l2b = group_t1.add_node(Lens::new(
        "T1 L2b",
        millimeter!(76.76954),
        millimeter!(-118.59590),
        millimeter!(14.0),
        &refr_index_hzf52,
    )?)?;
    let t1_l2c = group_t1.add_node(Lens::new(
        "T1 L2c",
        millimeter!(-63.45837),
        millimeter!(66.33014),
        millimeter!(7.68327),
        &refr_index_hzf2,
    )?)?;

    group_t1.connect_nodes(&t1_l1a, "output_1", &t1_l1b, "input_1", millimeter!(10.0))?;
    group_t1.connect_nodes(
        &t1_l1b,
        "output_1",
        &t1_l2a,
        "input_1",
        millimeter!(937.23608),
    )?;
    group_t1.connect_nodes(
        &t1_l2a,
        "output_1",
        &t1_l2b,
        "input_1",
        millimeter!(8.85423),
    )?;
    group_t1.connect_nodes(
        &t1_l2b,
        "output_1",
        &t1_l2c,
        "input_1",
        millimeter!(14.78269),
    )?;

    group_t1.map_input_port(&t1_l1a, "input_1", "input_1")?;
    group_t1.map_output_port(&t1_l2c, "output_1", "output_1")?;

    group_t1.set_expand_view(false)?;
    let t1 = scenery.add_node(group_t1)?;

    scenery.connect_nodes(&input_group, "output_1", &t1, "input_1", millimeter!(100.0))?;

    // Dichroic beam splitter + filters (1w/2w)

    let mut group_bs = NodeGroup::new("Dichroic beam splitter");

    // ideal spectrum
    let short_pass_spectrum = generate_filter_spectrum(
        nanometer!(400.0)..nanometer!(2000.0),
        nanometer!(1.0),
        &opossum::spectrum_helper::FilterType::ShortPassStep {
            cut_off: nanometer!(700.0),
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
    let mut node = IdealFilter::new("1w Longpass filter", &felh1000)?;
    node.set_aperture(&PortType::Input, "input_1", &a_1inch)?;
    let filter_1w = group_bs.add_node(node)?;
    group_bs.connect_nodes(
        &bs,
        "out2_trans2_refl1",
        &filter_1w,
        "input_1",
        Length::zero(),
    )?;

    // Long pass filter (2w)
    let fesh0700 = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/FESH0700_Transmission.csv",
    )?);
    let mut node = IdealFilter::new("2w Shortpass filter", &fesh0700)?;
    node.set_aperture(&PortType::Input, "input_1", &a_1inch)?;
    let filter_2w = group_bs.add_node(node)?;
    group_bs.connect_nodes(
        &bs,
        "out1_trans1_refl2",
        &filter_2w,
        "input_1",
        Length::zero(),
    )?;

    group_bs.map_input_port(&bs, "input_1", "input_1")?;
    group_bs.map_output_port(&filter_1w, "output_1", "output_1w")?;
    group_bs.map_output_port(&filter_2w, "output_1", "output_2w")?;

    let bs_group = scenery.add_node(group_bs)?;

    scenery.connect_nodes(&t1, "output_1", &bs_group, "input_1", millimeter!(100.0))?;
    // 1w branch

    // T2_1w
    let mut group_t2_1w = NodeGroup::new("T2 1w");
    let t2_1w_in = group_t2_1w.add_node(Lens::new(
        "T2 1w In",
        millimeter!(405.38435),
        millimeter!(-702.52114),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t2_1w_field = group_t2_1w.add_node(Lens::new(
        "T2 1w Field",
        millimeter!(179.59020),
        millimeter!(f64::INFINITY),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t2_1w_exit = group_t2_1w.add_node(Lens::new(
        "T2 1w Exit",
        millimeter!(f64::INFINITY),
        millimeter!(-202.81235),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;

    group_t2_1w.connect_nodes(
        &t2_1w_in,
        "output_1",
        &t2_1w_field,
        "input_1",
        millimeter!(442.29480),
    )?;
    group_t2_1w.connect_nodes(
        &t2_1w_field,
        "output_1",
        &t2_1w_exit,
        "input_1",
        millimeter!(429.20520),
    )?;

    group_t2_1w.map_input_port(&t2_1w_in, "input_1", "input_1")?;
    group_t2_1w.map_output_port(&t2_1w_exit, "output_1", "output_1")?;
    let t2_1w = scenery.add_node(group_t2_1w)?;

    // T3_1w
    let mut group_t3_1w = NodeGroup::new("T3 1w");

    let t3_1w_input = group_t3_1w.add_node(Lens::new(
        "T3 1w Input",
        millimeter!(f64::INFINITY),
        millimeter!(-417.35031),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t3_1w_exit = group_t3_1w.add_node(Lens::new(
        "T3 1w Exit",
        millimeter!(156.35054),
        millimeter!(f64::INFINITY),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_1w_12 = group_t3_1w.add_node(Dummy::new("1w d12"))?;
    group_t3_1w.connect_nodes(
        &t3_1w_input,
        "output_1",
        &t3_1w_exit,
        "input_1",
        millimeter!(1181.0000),
    )?;
    group_t3_1w.connect_nodes(
        &t3_1w_exit,
        "output_1",
        &d_1w_12,
        "input_1",
        millimeter!(279.86873),
    )?;

    group_t3_1w.map_input_port(&t3_1w_input, "input_1", "input_1")?;
    group_t3_1w.map_output_port(&d_1w_12, "output_1", "output_1")?;
    let t3_1w = scenery.add_node(group_t3_1w)?;

    scenery.connect_nodes(
        &bs_group,
        "output_1w",
        &t2_1w,
        "input_1",
        millimeter!(537.5190),
    )?;
    scenery.connect_nodes(
        &t2_1w,
        "output_1",
        &t3_1w,
        "input_1",
        millimeter!(664.58900),
    )?;

    let mut group_det_1w = NodeGroup::new("Detectors 1w");

    let det_prop = group_det_1w.add_node(RayPropagationVisualizer::new("Propagation", None)?)?;
    let det_wavefront_1w = group_det_1w.add_node(WaveFront::new("Wavefront"))?;
    let cambox_1w = group_det_1w.add_node(cambox_1w()?)?;
    let det_energy_1w =
        group_det_1w.add_node(EnergyMeter::new("Energy", Metertype::IdealEnergyMeter))?;

    group_det_1w.connect_nodes(
        &det_prop,
        "output_1",
        &det_wavefront_1w,
        "input_1",
        Length::zero(),
    )?;
    group_det_1w.connect_nodes(
        &det_wavefront_1w,
        "output_1",
        &det_energy_1w,
        "input_1",
        Length::zero(),
    )?;
    group_det_1w.connect_nodes(
        &det_energy_1w,
        "output_1",
        &cambox_1w,
        "input_1",
        Length::zero(),
    )?;

    group_det_1w.map_input_port(&det_prop, "input_1", "input_1")?;

    let det_1w = scenery.add_node(group_det_1w)?;
    scenery.connect_nodes(&t3_1w, "output_1", &det_1w, "input_1", Length::zero())?;

    // 2w branch

    // T2_2w
    let mut group_t2_2w = NodeGroup::new("T2 2w");

    let t2_2w_in = group_t2_2w.add_node(Lens::new(
        "T2 2w In",
        millimeter!(536.5733),
        millimeter!(-677.68238),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t2_2w_field = group_t2_2w.add_node(Lens::new(
        "T2 2w Field",
        millimeter!(208.48421),
        millimeter!(f64::INFINITY),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t2_2w_exit = group_t2_2w.add_node(Lens::new(
        "T2 2w Exit",
        millimeter!(-767.51217),
        millimeter!(-178.98988),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    group_t2_2w.connect_nodes(
        &t2_2w_in,
        "output_1",
        &t2_2w_field,
        "input_1",
        millimeter!(409.38829),
    )?;
    group_t2_2w.connect_nodes(
        &t2_2w_field,
        "output_1",
        &t2_2w_exit,
        "input_1",
        millimeter!(512.11171),
    )?;

    group_t2_2w.map_input_port(&t2_2w_in, "input_1", "input_1")?;
    group_t2_2w.map_output_port(&t2_2w_exit, "output_1", "output_1")?;
    let t2_2w = scenery.add_node(group_t2_2w)?;

    // T3_2w
    let mut group_t3_2w = NodeGroup::new("T3 2w");

    let t3_2w_input = group_t3_2w.add_node(Lens::new(
        "T3 2w Input",
        millimeter!(932.92634),
        millimeter!(-724.14405),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let t3_2w_exit = group_t3_2w.add_node(Lens::new(
        "T3 2w Exit",
        millimeter!(161.31174),
        millimeter!(-1069.52277),
        millimeter!(9.5),
        &refr_index_hk9l,
    )?)?;
    let d_2w_12 = group_t3_2w.add_node(Dummy::new("2w d12"))?;
    group_t3_2w.connect_nodes(
        &t3_2w_input,
        "output_1",
        &t3_2w_exit,
        "input_1",
        millimeter!(1181.0000),
    )?;
    group_t3_2w.connect_nodes(
        &t3_2w_exit,
        "output_1",
        &d_2w_12,
        "input_1",
        millimeter!(250.35850),
    )?;

    group_t3_2w.map_input_port(&t3_2w_input, "input_1", "input_1")?;
    group_t3_2w.map_output_port(&d_2w_12, "output_1", "output_1")?;
    let t3_2w = scenery.add_node(group_t3_2w)?;

    scenery.connect_nodes(
        &bs_group,
        "output_2w",
        &t2_2w,
        "input_1",
        millimeter!(474.589),
    )?;
    scenery.connect_nodes(
        &t2_2w,
        "output_1",
        &t3_2w,
        "input_1",
        millimeter!(622.09000),
    )?;

    // 2w detectors
    let mut group_det_2w = NodeGroup::new("Detectors 2w");

    let det_prop_2w = group_det_2w.add_node(RayPropagationVisualizer::new("Propagation", None)?)?;
    let det_wavefront_2w = group_det_2w.add_node(WaveFront::new("Wavefront"))?;
    let det_energy_2w =
        group_det_2w.add_node(EnergyMeter::new("Energy", Metertype::IdealEnergyMeter))?;
    let cambox_2w = group_det_2w.add_node(cambox_2w()?)?;

    group_det_2w.connect_nodes(
        &det_prop_2w,
        "output_1",
        &det_wavefront_2w,
        "input_1",
        Length::zero(),
    )?;
    group_det_2w.connect_nodes(
        &det_wavefront_2w,
        "output_1",
        &det_energy_2w,
        "input_1",
        Length::zero(),
    )?;
    group_det_2w.connect_nodes(
        &det_energy_2w,
        "output_1",
        &cambox_2w,
        "input_1",
        Length::zero(),
    )?;

    group_det_2w.map_input_port(&det_prop_2w, "input_1", "input_1")?;
    let det_2w = scenery.add_node(group_det_2w)?;

    scenery.connect_nodes(&t3_2w, "output_1", &det_2w, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(0);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/hhts.opm"))
}
