use opossum::{
    error::OpmResult,
    millimeter,
    nodes::{BeamSplitter, Dummy, EnergyMeter, FilterType, IdealFilter, Metertype, NodeGroup},
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
    let meter = EnergyMeter::new("Beamdump", Metertype::IdealEnergyMeter);
    let beam_dump = group.add_node(meter)?;

    let hhts_t1_pm = group.add_node(BeamSplitter::new("HHTS_T1_PM", &double_mirror)?)?;

    group.connect_nodes(d1, "output_1", mm15, "input_1", millimeter!(500.0))?;
    group.connect_nodes(
        mm15,
        "out1_trans1_refl2",
        window,
        "input_1",
        millimeter!(200.0),
    )?;
    group.connect_nodes(
        window,
        "output_1",
        hhts_t1_cm,
        "input_1",
        millimeter!(200.0),
    )?;
    group.connect_nodes(
        hhts_t1_cm,
        "out1_trans1_refl2",
        beam_dump,
        "input_1",
        millimeter!(100.0),
    )?;
    group.connect_nodes(
        hhts_t1_cm,
        "out2_trans2_refl1",
        hhts_t1_pm,
        "input_1",
        millimeter!(1000.0),
    )?;
    group.map_input_port(d1, "input_1", "input_1")?;
    group.map_output_port(hhts_t1_pm, "out2_trans2_refl1", "output_1")?;
    Ok(group)
}
