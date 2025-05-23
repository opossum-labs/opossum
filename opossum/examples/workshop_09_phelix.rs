use num::Zero;
use opossum::{
    analyzers::{AnalyzerType, RayTraceConfig},
    degree,
    energy_distributions::UniformDist,
    error::OpmResult,
    joule,
    lightdata::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder},
    millimeter, nanometer,
    nodes::{
        NodeGroup, NodeReference, ParaxialSurface, RayPropagationVisualizer, Source, ThinMirror,
        Wedge,
    },
    optic_node::{Alignable, OpticNode},
    position_distributions::Grid,
    refractive_index::RefrIndexConst,
    spectral_distribution::LaserLines,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;
use uom::si::f64::Length;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("PHELIX MainAmp");

    let light_data_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
        pos_dist: Grid::new((Length::zero(), millimeter!(60.0)), (1, 5))?.into(),
        energy_dist: UniformDist::new(joule!(1.0))?.into(),
        spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into(),
    });
    let mut src = Source::new("incoming rays", light_data_builder);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(src)?;

    let fused_silica = RefrIndexConst::new(1.5)?;
    let lens1 = ParaxialSurface::new("Input lens", millimeter!(1400.0))?;
    let i_l1 = scenery.add_node(lens1)?;
    let i_m1 = scenery.add_node(ThinMirror::new("mirror 1").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_m2 = scenery.add_node(ThinMirror::new("mirror 2").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let lens2 = ParaxialSurface::new("Input lens", millimeter!(7000.0))?;
    let i_l2 = scenery.add_node(lens2)?;

    let i_mm3 = scenery.add_node(ThinMirror::new("MM3").with_tilt(degree!(45.0, 0.0, 0.0))?)?;
    let i_mm2 = scenery.add_node(ThinMirror::new("MM2").with_tilt(degree!(45.0, 0.0, 0.0))?)?;

    let disk_1 = Wedge::new("disk 1", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a1 = scenery.add_node(disk_1)?;

    let disk_2 = Wedge::new("disk 2", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a2 = scenery.add_node(disk_2)?;

    let disk_3 = Wedge::new("disk 3", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a3 = scenery.add_node(disk_3)?;

    let disk_4 = Wedge::new("disk 4", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a4 = scenery.add_node(disk_4)?;

    let disk_5 = Wedge::new("disk 5", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a5 = scenery.add_node(disk_5)?;

    let disk_6 = Wedge::new("disk 6", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a6 = scenery.add_node(disk_6)?;

    let disk_7 = Wedge::new("disk 7", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a7 = scenery.add_node(disk_7)?;

    let disk_8 = Wedge::new("disk 8", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a8 = scenery.add_node(disk_8)?;

    let disk_9 = Wedge::new("disk 9", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(56.0, 0.0, 0.0))?;
    let i_a9 = scenery.add_node(disk_9)?;

    let disk_10 = Wedge::new("disk 10", millimeter!(45.0), degree!(0.0), &fused_silica)?
        .with_tilt(degree!(-56.0, 0.0, 0.0))?;
    let i_a10 = scenery.add_node(disk_10)?;

    let i_mm1 = scenery.add_node(ThinMirror::new("MM2").with_tilt(degree!(-0.5, 0.0, 0.0))?)?;

    let mut r_a10 = NodeReference::from_node(&scenery.node(i_a10)?);
    r_a10.set_inverted(true)?;
    let i_r_a10 = scenery.add_node(r_a10)?;

    let mut r_a9 = NodeReference::from_node(&scenery.node(i_a9)?);
    r_a9.set_inverted(true)?;
    let i_r_a9 = scenery.add_node(r_a9)?;

    let mut r_a8 = NodeReference::from_node(&scenery.node(i_a8)?);
    r_a8.set_inverted(true)?;
    let i_r_a8 = scenery.add_node(r_a8)?;

    let mut r_a7 = NodeReference::from_node(&scenery.node(i_a7)?);
    r_a7.set_inverted(true)?;
    let i_r_a7 = scenery.add_node(r_a7)?;

    let mut r_a6 = NodeReference::from_node(&scenery.node(i_a6)?);
    r_a6.set_inverted(true)?;
    let i_r_a6 = scenery.add_node(r_a6)?;

    let mut r_a5 = NodeReference::from_node(&scenery.node(i_a5)?);
    r_a5.set_inverted(true)?;
    let i_r_a5 = scenery.add_node(r_a5)?;

    let mut r_a4 = NodeReference::from_node(&scenery.node(i_a4)?);
    r_a4.set_inverted(true)?;
    let i_r_a4 = scenery.add_node(r_a4)?;

    let mut r_a3 = NodeReference::from_node(&scenery.node(i_a3)?);
    r_a3.set_inverted(true)?;
    let i_r_a3 = scenery.add_node(r_a3)?;

    let mut r_a2 = NodeReference::from_node(&scenery.node(i_a2)?);
    r_a2.set_inverted(true)?;
    let i_r_a2 = scenery.add_node(r_a2)?;

    let mut r_a1 = NodeReference::from_node(&scenery.node(i_a1)?);
    r_a1.set_inverted(true)?;
    let i_r_a1 = scenery.add_node(r_a1)?;

    let mut r_mm2 = NodeReference::from_node(&scenery.node(i_mm2)?);
    r_mm2.set_inverted(true)?;
    let i_r_mm2 = scenery.add_node(r_mm2)?;

    let mut r_mm3 = NodeReference::from_node(&scenery.node(i_mm3)?);
    r_mm3.set_inverted(true)?;
    let i_r_mm3 = scenery.add_node(r_mm3)?;

    let mut r_l2 = NodeReference::from_node(&scenery.node(i_l2)?);
    r_l2.set_inverted(true)?;
    let i_r_l2 = scenery.add_node(r_l2)?;

    let lens3 = ParaxialSurface::new("Exit lens", millimeter!(7000.0))?;
    let i_l3 = scenery.add_node(lens3)?;

    let i_mm4 = scenery.add_node(ThinMirror::new("MM4").with_tilt(degree!(-45.0, 0.0, 0.0))?)?;

    let mut ray_prop_vis = RayPropagationVisualizer::new("propagation", None)?;
    ray_prop_vis.set_property("ray transparency", 1.0.into())?;
    let i_sd3 = scenery.add_node(ray_prop_vis)?;

    scenery.connect_nodes(i_src, "output_1", i_l1, "input_1", millimeter!(1500.0))?;
    scenery.connect_nodes(i_l1, "output_1", i_m1, "input_1", millimeter!(600.0))?;
    scenery.connect_nodes(i_m1, "output_1", i_m2, "input_1", millimeter!(600.0))?;
    scenery.connect_nodes(i_m2, "output_1", i_l2, "input_1", millimeter!(7200.0))?;
    scenery.connect_nodes(i_l2, "output_1", i_mm3, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_mm3, "output_1", i_mm2, "input_1", millimeter!(1500.0))?;

    // disks forward
    scenery.connect_nodes(i_mm2, "output_1", i_a1, "input_1", millimeter!(1000.0))?;
    scenery.connect_nodes(i_a1, "output_1", i_a2, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_a2, "output_1", i_a3, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_a3, "output_1", i_a4, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_a4, "output_1", i_a5, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_a5, "output_1", i_a6, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_a6, "output_1", i_a7, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_a7, "output_1", i_a8, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_a8, "output_1", i_a9, "input_1", millimeter!(800.0))?;
    scenery.connect_nodes(i_a9, "output_1", i_a10, "input_1", millimeter!(500.0))?;
    scenery.connect_nodes(i_a10, "output_1", i_mm1, "input_1", millimeter!(800.0))?;

    // disks backward
    scenery.connect_nodes(i_mm1, "output_1", i_r_a10, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a10, "input_1", i_r_a9, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a9, "input_1", i_r_a8, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a8, "input_1", i_r_a7, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a7, "input_1", i_r_a6, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a6, "input_1", i_r_a5, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a5, "input_1", i_r_a4, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a4, "input_1", i_r_a3, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a3, "input_1", i_r_a2, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_a2, "input_1", i_r_a1, "output_1", millimeter!(0.0))?;

    scenery.connect_nodes(i_r_a1, "input_1", i_r_mm2, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_mm2, "input_1", i_r_mm3, "output_1", millimeter!(0.0))?;
    scenery.connect_nodes(i_r_mm3, "input_1", i_r_l2, "output_1", millimeter!(0.0))?;

    scenery.connect_nodes(i_r_l2, "input_1", i_l3, "input_1", millimeter!(14000.0))?;
    scenery.connect_nodes(i_l3, "output_1", i_mm4, "input_1", millimeter!(600.0))?;

    scenery.connect_nodes(i_mm4, "output_1", i_sd3, "input_1", millimeter!(2000.0))?;

    let mut doc = OpmDocument::new(scenery);
    doc.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    doc.save_to_file(Path::new("./opossum/playground/workshop_09_phelix.opm"))
}
