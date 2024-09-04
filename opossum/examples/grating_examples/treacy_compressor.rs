use opossum::{
    degree,
    error::OpmResult,
    millimeter,
    nodes::{NodeGroup, ReflectiveGrating},
    num_per_mm, radian,
};
use uom::si::f64::Length;

pub fn treacy_compressor(alignment_wvl: Length) -> OpmResult<NodeGroup> {
    let mut cb = NodeGroup::new("Treacy compressor");

    let i_g1 = cb.add_node(
        ReflectiveGrating::new("grating 1", num_per_mm!(1740.), -1)?
            .with_rot_from_littrow(alignment_wvl, degree!(-4.))?,
    )?;

    let i_g2 = cb.add_node(
        ReflectiveGrating::new("grating 2", num_per_mm!(1740.), -1)?
            .to_rot_from_littrow(alignment_wvl, degree!(-4.) + radian!(10e-6))?,
    )?;

    let i_g3 = cb.add_node(
        ReflectiveGrating::new("grating 3", num_per_mm!(1740.), 1)?
            .with_rot_from_littrow(alignment_wvl, degree!(4.))?,
    )?;

    let i_g4 = cb.add_node(
        ReflectiveGrating::new("grating 4", num_per_mm!(1740.), 1)?
            .to_rot_from_littrow(alignment_wvl, degree!(4.))?,
    )?;

    cb.connect_nodes(i_g1, "diffracted", i_g2, "input", millimeter!(1000.0))?;
    cb.connect_nodes(i_g2, "diffracted", i_g3, "input", millimeter!(2500.0))?;
    cb.connect_nodes(i_g3, "diffracted", i_g4, "input", millimeter!(1000.0))?;

    cb.map_input_port(i_g1, "input", "input")?;
    cb.map_output_port(i_g4, "diffracted", "output")?;

    Ok(cb)
}
