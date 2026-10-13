use opossum_core::{
    apertures::{CircleShape, RectangleShape},
    core_optics::NodeAttrExt,
    geometry::body::CLEAR_APERTURE,
    prelude::*,
};
use uom::si::f64::Length;

pub fn folded_martinez_paraxial_lens(
    telescope_distance: Length,
    alignment_wvl: Length,
) -> OpmResult<NodeGroup> {
    //////////////////////////////////////////
    //       FoldedMartinez Stretcher       //
    //////////////////////////////////////////
    let mut cb = NodeGroup::new("Martinez stretcher");

    // The grating takes the dispersed beam on its return passes, up to 65 mm off its center.
    let mut grating = ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
        .with_rot_from_littrow(alignment_wvl, degree!(-4.))?;
    grating.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(RectangleShape::new(millimeter!(140.0), millimeter!(60.0))?).into(),
    )?;
    let i_g1 = cb.add_node(grating)?;
    // focal length = 996.7 mm (Thorlabs LA1779-B); the dispersed beam passes the lens up to 73 mm
    // off its center: 6 inches.
    let mut lens = ParaxialSurface::new("paraxial lens", telescope_distance)?
        .with_decenter(centimeter!(0., 1., 0.))?;
    lens.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(76.2))?).into(),
    )?;
    let lens1 = cb.add_node(lens)?;

    // The spectrum spreads up to 59 mm across the mirror in the focal plane: 5 inches.
    let mut mirror = ThinMirror::new("mirr").align_like_node_at_distance(lens1, telescope_distance);
    mirror.set_property(
        CLEAR_APERTURE,
        ApertureShape::from(CircleShape::new(millimeter!(63.5))?).into(),
    )?;
    let mir_1 = cb.add_node(mirror)?;
    let mir_1_ref = cb.add_node(NodeReference::from_node(&cb.node(mir_1)?)?)?;
    let mut lens_1_ref1 = NodeReference::from_node(&cb.node(lens1)?)?;
    lens_1_ref1.set_inverted(true)?;
    let lens_1_ref1 = cb.add_node(lens_1_ref1)?;
    let lens_1_ref2 = cb.add_node(NodeReference::from_node(&cb.node(lens1)?)?)?;
    let mut lens_1_ref3 = NodeReference::from_node(&cb.node(lens1)?)?;
    lens_1_ref3.set_inverted(true)?;
    let lens_1_ref3 = cb.add_node(lens_1_ref3)?;
    let mut g1ref1 = NodeReference::from_node(&cb.node(i_g1)?)?;
    g1ref1.set_inverted(true)?;
    let g1ref1 = cb.add_node(g1ref1)?;
    let g1ref2 = cb.add_node(NodeReference::from_node(&cb.node(i_g1)?)?)?;
    let mut g1ref3 = NodeReference::from_node(&cb.node(i_g1)?)?;
    g1ref3.set_inverted(true)?;
    let g1ref3 = cb.add_node(g1ref3)?;
    // The spatially chirped beam is up to 22 mm off the centers of the roof mirrors: 2 inches.
    let roof_mirror = |name: &str| -> OpmResult<ThinMirror> {
        let mut mirror = ThinMirror::new(name).with_tilt(degree!(-45., 0., 0.))?;
        mirror.set_property(
            CLEAR_APERTURE,
            ApertureShape::from(CircleShape::new(millimeter!(25.4))?).into(),
        )?;
        Ok(mirror)
    };
    let retro_mir1 = cb.add_node(roof_mirror("retro_mir1")?)?;
    let retro_mir2 = cb.add_node(roof_mirror("retro_mir2")?)?;

    //first grating pass up to 0° mirror
    cb.connect_nodes(i_g1, "output_1", lens1, "input_1", millimeter!(800.))?;
    cb.connect_nodes(lens1, "output_1", mir_1, "input_1", millimeter!(100.0))?;

    // second grating pass pass up to rooftop mirror
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
    cb.connect_nodes(
        retro_mir1,
        "output_1",
        retro_mir2,
        "input_1",
        millimeter!(5.0),
    )?;
    cb.connect_nodes(retro_mir2, "output_1", g1ref2, "input_1", millimeter!(10.0))?;

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
    cb.map_output_port(g1ref3, "input_1", "output_1")?;

    Ok(cb)
}
