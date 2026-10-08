use opossum_core::{
    apertures::RectangleShape,
    core_optics::{NodeAttrExt, node_attr::HasNodeAttr},
    geometry::body::CLEAR_APERTURE,
    prelude::*,
};
use uom::si::f64::Length;

pub fn treacy_compressor(alignment_wvl: Length) -> OpmResult<NodeGroup> {
    let mut cb = NodeGroup::new("Treacy compressor");

    let grating_1 = ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
        .with_rot_from_littrow(alignment_wvl, degree!(-4.0))?;
    let rot = grating_1
        .node_attr()
        .alignment()
        .as_ref()
        .unwrap()
        .rotation();
    let grating_1 = grating_1.with_tilt(rot)?;

    let i_g1 = cb.add_node(grating_1)?;

    // Gratings 2 and 3 take the dispersed beam, up to 145 mm off their centers along the
    // dispersion: they are 300 mm wide and 30 mm high.
    let wide_grating: ApertureShape =
        RectangleShape::new(millimeter!(300.0), millimeter!(30.0))?.into();
    let mut grating_2 = ReflectiveGrating::new("grating 2", num_per_mm!(1740.), -1)?
        .to_rot_from_littrow(alignment_wvl, degree!(-4.001))?;
    grating_2.set_property(CLEAR_APERTURE, wide_grating.clone().into())?;
    let i_g2 = cb.add_node(grating_2)?;

    let mut grating_3 = ReflectiveGrating::new("grating 3", num_per_mm!(1740.), 1)?
        .with_rot_from_littrow(alignment_wvl, degree!(4.))?;
    grating_3.set_property(CLEAR_APERTURE, wide_grating.into())?;
    let i_g3 = cb.add_node(grating_3)?;

    let grating_4 = ReflectiveGrating::new("grating 4", num_per_mm!(1740.), 1)?
        .to_rot_from_littrow(alignment_wvl, degree!(4.0))?;
    let rot = grating_4
        .node_attr()
        .alignment()
        .as_ref()
        .unwrap()
        .rotation();
    let grating_4 = grating_4.with_tilt(rot)?;

    let i_g4 = cb.add_node(grating_4)?;

    cb.connect_nodes(i_g1, "output_1", i_g2, "input_1", millimeter!(1000.0))?;
    cb.connect_nodes(i_g2, "output_1", i_g3, "input_1", millimeter!(2500.0))?;
    cb.connect_nodes(i_g3, "output_1", i_g4, "input_1", millimeter!(1000.0))?;

    cb.map_input_port(i_g1, "input_1", "input_1")?;
    cb.map_output_port(i_g4, "output_1", "output_1")?;
    cb.set_expand_view(true)?;
    Ok(cb)
}
