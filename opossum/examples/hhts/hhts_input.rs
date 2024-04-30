use opossum::{
    aperture::CircleConfig,
    error::OpmResult,
    millimeter,
    nodes::{BeamSplitter, Dummy, EnergyMeter, FilterType, IdealFilter, Metertype, NodeGroup},
    optical::Optical,
    ray::SplittingConfig,
    spectrum::Spectrum,
};
pub fn hhts_input() -> OpmResult<NodeGroup> {
    let dichroic_mirror = SplittingConfig::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/MM15_Transmission.csv",
    )?);
    let window_filter = FilterType::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/HHTS_W1_Transmission.csv",
    )?);
    let double_mirror = SplittingConfig::Spectrum(Spectrum::from_csv(
        "opossum/examples/hhts/HHTS_T1_PM_Transmission.csv",
    )?);
    let mut group = NodeGroup::new("HHTS Input");
    let d1 = group.add_node(Dummy::new("d1"))?;
    let mm15 = group.add_node(BeamSplitter::new("MM15", &dichroic_mirror)?)?;
    let window = group.add_node(IdealFilter::new("window", &window_filter)?)?;
    let hhts_t1_cm = group.add_node(BeamSplitter::new("HHTS_T1_CM", &dichroic_mirror)?)?;
    let mut meter = EnergyMeter::new("Beamdump", Metertype::IdealEnergyMeter);
    let circle_config = CircleConfig::new(millimeter!(10.0), millimeter!(0.0, 0.0))?;
    meter.set_input_aperture(
        "in1",
        &opossum::aperture::Aperture::BinaryCircle(circle_config),
    )?;
    let beam_dump = group.add_node(meter)?;

    let hhts_t1_pm = group.add_node(BeamSplitter::new("HHTS_T1_PM", &double_mirror)?)?;
    let d6 = group.add_node(Dummy::new("d6"))?;

    group.connect_nodes(d1, "rear", mm15, "input1", millimeter!(500.0))?;
    group.connect_nodes(
        mm15,
        "out1_trans1_refl2",
        window,
        "front",
        millimeter!(200.0),
    )?;
    group.connect_nodes(window, "rear", hhts_t1_cm, "input1", millimeter!(200.0))?;
    group.connect_nodes(
        hhts_t1_cm,
        "out1_trans1_refl2",
        beam_dump,
        "in1",
        millimeter!(100.0),
    )?;
    group.connect_nodes(
        hhts_t1_cm,
        "out2_trans2_refl1",
        hhts_t1_pm,
        "input1",
        millimeter!(1000.0),
    )?;
    group.connect_nodes(
        hhts_t1_pm,
        "out2_trans2_refl1",
        d6,
        "front",
        millimeter!(100.0),
    )?;

    group.map_input_port(d1, "front", "input")?;
    group.map_output_port(d6, "rear", "output")?;
    Ok(group)
}
