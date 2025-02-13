use opossum::{
    analyzers::{AnalyzerType, GhostFocusConfig},
    coatings::CoatingType,
    degree,
    energy_distributions::General2DGaussian,
    error::OpmResult,
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    nodes::{
        round_collimated_ray_source, Lens, NodeGroup, RayPropagationVisualizer, Source,
        SpotDiagram, ThinMirror, Wedge,
    },
    optic_node::{Alignable, OpticNode},
    optic_ports::PortType,
    position_distributions::{HexagonalTiling, Hexapolar},
    radian,
    rays::Rays,
    refractive_index::RefrIndexConst,
    utils::geom_transformation::Isometry,
    OpmDocument,
};
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::default();
    scenery.node_attr_mut().set_name("Folded Telescope");

    let rays = Rays::new_collimated(
        nanometer!(1000.0),
        &General2DGaussian::new(
            joule!(2.),
            millimeter!(0., 0.),
            millimeter!(8., 8.),
            5.,
            radian!(0.),
            false,
        )?,
        &HexagonalTiling::new(millimeter!(15.0), 25, millimeter!(0.0, 0.))?,
    )?;

    let light = LightData::Geometric(rays);
    let mut src = Source::new("collimated ray source", &light);
    src.set_isometry(Isometry::identity())?;
    let i_src = scenery.add_node(&src)?;
    let mut lens = Lens::default();
    lens.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.05 },
    )?;
    lens.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingType::ConstantR { reflectivity: 0.05 },
    )?;
    let i_l = scenery.add_node(&lens)?;

    let mut lens2 = Lens::default();
    lens2.set_coating(
        &PortType::Input,
        "input_1",
        &CoatingType::ConstantR { reflectivity: 0.2 },
    )?;
    lens2.set_coating(
        &PortType::Output,
        "output_1",
        &CoatingType::ConstantR { reflectivity: 0.2 },
    )?;
    lens2.set_coating(&PortType::Input, "input_1", &CoatingType::Fresnel)?;
    lens2.set_coating(&PortType::Output, "output_1", &CoatingType::Fresnel)?;
    let i_l2 = scenery.add_node(&lens2)?;

    // let mut mgroup = NodeGroup::new("mirror group");
    let mir1 = scenery.add_node(&ThinMirror::new("mirror 1").with_tilt(degree!(45., 0., 0.))?)?;
    let mir2 = scenery.add_node(&ThinMirror::new("mirror 2").with_tilt(degree!(45., 0., 0.))?)?;
    let mir3 = scenery.add_node(&ThinMirror::new("mirror 3").with_tilt(degree!(-45., 0., 0.))?)?;
    let mir4 = scenery.add_node(&ThinMirror::new("mirror 4").with_tilt(degree!(-45., 0., 0.))?)?;

    // mgroup.connect_nodes(mir1, "output_1", mir2, "input_1", millimeter!(200.0))?;
    // mgroup.connect_nodes(mir2, "output_1", mir3, "input_1", millimeter!(300.0))?;
    // mgroup.connect_nodes(mir3, "output_1", mir4, "input_1", millimeter!(200.0))?;

    // mgroup.map_input_port(mir1, "input_1", "input_1");
    // mgroup.map_output_port(mir4, "output_1", "output_1");
    // let mg = scenery.add_node(&mgroup)?;

    scenery.connect_nodes(i_src, "output_1", i_l, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(i_l, "output_1", mir1, "input_1", millimeter!(150.0))?;
    scenery.connect_nodes(mir1, "output_1", mir2, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(mir2, "output_1", mir3, "input_1", millimeter!(300.0))?;
    scenery.connect_nodes(mir3, "output_1", mir4, "input_1", millimeter!(200.0))?;
    scenery.connect_nodes(mir4, "output_1", i_l2, "input_1", millimeter!(150.0))?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = GhostFocusConfig::default();
    config.set_max_bounces(1);
    doc.add_analyzer(AnalyzerType::GhostFocus(config));
    doc.save_to_file(Path::new("./opossum/playground/ghost_focus.opm"))
}
