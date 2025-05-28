use nalgebra::Vector3;
use opossum::analyzers::{AnalyzerType, RayTraceConfig};
use opossum::lightdata::light_data_builder::LightDataBuilder;
use opossum::lightdata::ray_data_builder::RayDataBuilder;
use opossum::nodes::{NodeGroup, NodeReference, ParaxialSurface, SpotDiagram, ThinMirror};
use opossum::optic_node::Alignable;
use opossum::refractive_index::{RefrIndexConst, RefractiveIndex};
use opossum::OpmDocument;
use opossum::{
    energy_distributions::UniformDist,
    error::OpmResult,
    joule, millimeter, nanometer,
    nodes::{Lens, RayPropagationVisualizer, Source},
    optic_node::OpticNode,
    position_distributions::Hexapolar,
    refractive_index::RefrIndexSellmeier1,
    spectral_distribution::Gaussian,
    utils::geom_transformation::Isometry,
};

mod folded_martinez;
use folded_martinez::folded_martinez;
mod folded_martinez_longer_f;
use folded_martinez_longer_f::folded_martinez_longer_f;
mod detector_group;
use detector_group::detector_group;
mod treacy_compressor;
use treacy_compressor::treacy_compressor;
mod folded_martinez_paraxial_lens;
use folded_martinez_paraxial_lens::folded_martinez_paraxial_lens;
use std::path::Path;

fn main() -> OpmResult<()> {
    // let tel_dist = millimeter!(2467.96162);
    let tel_dist = millimeter!(1015.515);
    let alignment_wvl = nanometer!(1054.);
    let nbk7 = RefrIndexSellmeier1::new(
        1.039612120,
        0.231792344,
        1.010469450,
        0.00600069867,
        0.0200179144,
        103.5606530,
        nanometer!(300.)..nanometer!(1200.),
    )?;
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(millimeter!(1.), 4)?.into(),
        energy_dist: UniformDist::new(joule!(1.))?.into(),
        spect_dist: Gaussian::new(
            (nanometer!(1040.), nanometer!(1068.)),
            30,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?
        .into(),
    });
    let mut src = Source::new("collimated ray source", light_data_builder);
    src.set_alignment_wavelength(alignment_wvl)?;
    src.set_isometry(Isometry::identity())?;
    ////////////////////////////////////
    //  4 grating compressor example  //
    ////////////////////////////////////
    let mut scenery = NodeGroup::new("treacy compressor");

    let i_src = scenery.add_node(src.clone())?;
    let compressor_node = scenery.add_node(treacy_compressor(alignment_wvl)?)?;
    
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        compressor_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        compressor_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(400.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/treacy_compressor.opm"))?;

    //////////////////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope                   //
    //       Lenses are spherical + chromatic lens                      //
    //       Telescope length slightly misaligned (0.5% from ideal)     //
    //////////////////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(1015.515 * 0.995);
    let mut scenery = NodeGroup::new("non-ideal folded Martinez stretcher");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node =
        scenery.add_node(folded_martinez(telescope_distance, &nbk7, alignment_wvl)?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(0.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/nonideal_folded_martinez.opm",
    ))?;

    //////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope       //
    //       Lenses are spherical + chromatic lens          //
    //       Telescope length perfectly aligned             //
    //////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(1017.14885);
    let mut scenery = NodeGroup::new("ideal folded Martinez stretcher");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node =
        scenery.add_node(folded_martinez(telescope_distance, &nbk7, alignment_wvl)?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(1500.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/ideal_folded_martinez.opm"))?;

    //////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope       //
    //       Lenses are spherical + chromatic lens          //
    //       Telescope length perfectly aligned             //
    //////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(1015.515);
    let mut scenery = NodeGroup::new("ideal folded Martinez stretcher circle of least conf");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node =
        scenery.add_node(folded_martinez(telescope_distance, &nbk7, alignment_wvl)?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(1500.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/ideal_folded_martinez_circle_of_least_conf.opm",
    ))?;

    //////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope       //
    //       Lenses are spherical + chromatic lens          //
    //       Telescope length perfectly aligned but longer f             //
    //////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(2467.1);
    let mut scenery = NodeGroup::new("ideal folded Martinez stretcher longer f");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node = scenery.add_node(folded_martinez_longer_f(
        telescope_distance,
        &nbk7,
        alignment_wvl,
    )?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(1500.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/ideal_folded_martinez_longer_f.opm",
    ))?;

    //////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope       //
    //       Lenses are spherical + achromatic lens         //
    //       Telescope length perfectly aligned             //
    //////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(1017.14885);
    let mut scenery = NodeGroup::new("achromatic ideal folded Martinez stretcher");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node = scenery.add_node(folded_martinez(
        telescope_distance,
        &RefrIndexConst::new(nbk7.get_refractive_index(nanometer!(1054.))?)?,
        alignment_wvl,
    )?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(1500.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/achromat_ideal_folded_martinez.opm",
    ))?;

    //////////////////////////////////////////////////////////
    //       Martinez Stretcher with folded telescope       //
    //       Lenses are perfectly paraxial lenses           //
    //       Telescope length perfectly aligned             //
    //////////////////////////////////////////////////////////
    let telescope_distance = millimeter!(1017.14885);
    let mut scenery = NodeGroup::new("paraxial folded Martinez stretcher");

    let i_src = scenery.add_node(src.clone())?;
    let stretcher_node = scenery.add_node(folded_martinez_paraxial_lens(
        telescope_distance,
        alignment_wvl,
    )?)?;
    let detectors = scenery.add_node(detector_group()?)?;

    scenery.connect_nodes(
        i_src,
        "output_1",
        stretcher_node,
        "input_1",
        millimeter!(400.0),
    )?;
    scenery.connect_nodes(
        stretcher_node,
        "output_1",
        detectors,
        "input_1",
        millimeter!(1500.),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new(
        "./opossum/playground/achromat_ideal_folded_martinez.opm",
    ))?;

    ////////////////////////////
    //       Telescope        //
    ////////////////////////////
    let alignment_wvl = nanometer!(1054.);
    let nbk7 = RefrIndexSellmeier1::new(
        1.039612120,
        0.231792344,
        1.010469450,
        0.00600069867,
        0.0200179144,
        103.5606530,
        nanometer!(300.)..nanometer!(1200.),
    )?;
    let mut scenery = NodeGroup::new("telescope");
    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Hexapolar::new(millimeter!(50.), 8)?.into(),
        energy_dist: UniformDist::new(joule!(1.))?.into(),
        spect_dist: Gaussian::new(
            (nanometer!(1054.), nanometer!(1068.)),
            1,
            nanometer!(1054.),
            nanometer!(8.),
            1.,
        )?
        .into(),
    });
    let mut src = Source::new("collimated ray source", light_data_builder);
    src.set_alignment_wavelength(alignment_wvl)?;
    src.set_isometry(Isometry::identity())?;

    let i_src = scenery.add_node(src)?;
    // focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens1 = scenery.add_node(Lens::new(
        "Lens 1",
        millimeter!(515.1),
        millimeter!(f64::INFINITY),
        millimeter!(2.1),
        &nbk7,
    )?)?;
    let mir_1 =
        scenery.add_node(ThinMirror::new("mirr").align_like_node_at_distance(lens1, tel_dist))?;
    let mut lens_1_ref1 = NodeReference::from_node(&scenery.node(lens1)?);
    lens_1_ref1.set_inverted(true)?;
    let lens_1_ref1 = scenery.add_node(lens_1_ref1)?;

    let paraxial_lens = scenery.add_node(ParaxialSurface::new("ideal lens", millimeter!(500.))?)?;
    let spot_diag = scenery.add_node(SpotDiagram::new("spot diagram"))?;
    let i_prop_vis = scenery.add_node(RayPropagationVisualizer::new(
        "Ray_positions",
        Some(Vector3::y()),
    )?)?;
    scenery.connect_nodes(i_src, "output_1", lens1, "input_1", millimeter!(100.0))?;
    scenery.connect_nodes(lens1, "output_1", mir_1, "input_1", millimeter!(1017.14885))?;
    scenery.connect_nodes(
        mir_1,
        "output_1",
        lens_1_ref1,
        "output_1",
        millimeter!(1017.14885),
    )?;
    scenery.connect_nodes(
        lens_1_ref1,
        "input_1",
        paraxial_lens,
        "input_1",
        millimeter!(500.0),
    )?;
    scenery.connect_nodes(
        paraxial_lens,
        "output_1",
        spot_diag,
        "input_1",
        millimeter!(500.0),
    )?;
    scenery.connect_nodes(
        spot_diag,
        "output_1",
        i_prop_vis,
        "input_1",
        millimeter!(0.0),
    )?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/telescope.opm"))
}
