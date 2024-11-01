use opossum::{
    centimeter, degree,
    error::OpmResult,
    millimeter,
    nodes::{Lens, NodeGroup, NodeReference, ReflectiveGrating, ThinMirror},
    num_per_mm,
    optic_node::{Alignable, OpticNode},
    refractive_index::RefractiveIndex,
};
use uom::si::f64::Length;

pub fn folded_martinez(
    telescope_distance: Length,
    refr_index: &dyn RefractiveIndex,
    alignment_wvl: Length,
) -> OpmResult<NodeGroup> {
    //////////////////////////////////////////
    //       FoldedMartinez Stretcher       //
    //////////////////////////////////////////
    let mut cb = NodeGroup::new("Folded Martinez stretcher");

    let i_g1 = cb.add_node(
        &ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
            .with_rot_from_littrow(alignment_wvl, degree!(-4.))?,
    )?;
    // focal length = 996.7 mm (Thorlabs LA1779-B)
    let lens1 = cb.add_node(
        &Lens::new(
            "Lens 1",
            millimeter!(515.1),
            millimeter!(f64::INFINITY),
            millimeter!(2.1),
            refr_index,
        )?
        .with_decenter(centimeter!(0., 0., 0.))?,
    )?;

    let mir_1 = cb.add_node(
        &ThinMirror::new("mirr").align_like_node_at_distance(lens1, telescope_distance),
    )?;
    let mir_1_ref = cb.add_node(&NodeReference::from_node(&cb.node(mir_1)?))?;
    let mut lens_1_ref1 = NodeReference::from_node(&cb.node(lens1)?);
    lens_1_ref1.set_inverted(true)?;
    let lens_1_ref1 = cb.add_node(&lens_1_ref1)?;
    let lens_1_ref2 = cb.add_node(&NodeReference::from_node(&cb.node(lens1)?))?;
    let mut lens_1_ref3 = NodeReference::from_node(&cb.node(lens1)?);
    lens_1_ref3.set_inverted(true)?;
    let lens_1_ref3 = cb.add_node(&lens_1_ref3)?;
    let mut g1ref1 = NodeReference::from_node(&cb.node(i_g1)?);
    g1ref1.set_inverted(true)?;
    let g1ref1 = cb.add_node(&g1ref1)?;
    let g1ref2 = cb.add_node(&NodeReference::from_node(&cb.node(i_g1)?))?;
    let mut g1ref3 = NodeReference::from_node(&cb.node(i_g1)?);
    g1ref3.set_inverted(true)?;
    let g1ref3 = cb.add_node(&g1ref3)?;
    let retro_mir1 = cb.add_node(&ThinMirror::new("retro_mir1"))?;
    // let retro_mir1 =
    //     cb.add_node(&ThinMirror::new("retro_mir1").with_tilt(degree!(-45., 0., 0.))?)?;
    // let retro_mir2 =
    //     cb.add_node(&ThinMirror::new("retro_mir2").with_tilt(degree!(-45., 0., 0.))?)?;

    //first grating pass up to 0° mirror
    cb.connect_nodes(
        i_g1,
        "output_1",
        lens1,
        "input_1",
        telescope_distance - millimeter!(200.),
    )?;
    cb.connect_nodes(lens1, "output_1", mir_1, "input_1", millimeter!(100.0))?;

    //second grating pass pass up to rooftop mirror
    cb.connect_nodes(
        mir_1,
        "output_1",
        lens_1_ref1,
        "output_1",
        millimeter!(100.0),
    )?;
    cb.connect_nodes(
        lens_1_ref1,
        "input_1",
        g1ref1,
        "output_1",
        millimeter!(100.0),
    )?;
    cb.connect_nodes(g1ref1, "input_1", retro_mir1, "input_1", telescope_distance)?;
    cb.connect_nodes(retro_mir1, "output_1", g1ref2, "input_1", millimeter!(10.0))?;

    // // cb.connect_nodes(
    // //     retro_mir1,
    // //     "output_1",
    // //     retro_mir2,
    // //     "input_1",
    // //     millimeter!(5.0),
    // // )?;
    // // cb.connect_nodes(retro_mir2, "output_1", g1ref2, "input_1", millimeter!(10.0))?;

    //third grating pass pass up to 0° mirror
    cb.connect_nodes(
        g1ref2,
        "output_1",
        lens_1_ref2,
        "input_1",
        millimeter!(1500.0),
    )?;
    cb.connect_nodes(
        lens_1_ref2,
        "output_1",
        mir_1_ref,
        "input_1",
        millimeter!(100.0),
    )?;

    //fourth grating pass up to last grating interaction
    cb.connect_nodes(
        mir_1_ref,
        "output_1",
        lens_1_ref3,
        "output_1",
        millimeter!(100.0),
    )?;
    cb.connect_nodes(
        lens_1_ref3,
        "input_1",
        g1ref3,
        "output_1",
        millimeter!(100.0),
    )?;

    cb.map_input_port(i_g1, "input_1", "input_1")?;
    // cb.map_output_port(g1ref3, "input_1", "output_1")?;
    cb.map_output_port(g1ref3, "input_1", "output_1")?;

    Ok(cb)
}
