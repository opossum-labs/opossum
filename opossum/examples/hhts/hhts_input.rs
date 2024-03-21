use opossum::{
    error::OpmResult,
    nodes::{
        BeamSplitter, EnergyMeter, FilterType, IdealFilter, Metertype, NodeGroup, Propagation,
    },
    ray::SplittingConfig,
    spectrum::Spectrum,
};
use uom::si::{f64::Length, length::millimeter};

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
    let d1 = group.add_node(Propagation::new("d1", Length::new::<millimeter>(500.0))?)?;
    let mm15 = group.add_node(BeamSplitter::new("MM15", &dichroic_mirror)?)?;
    let d2 = group.add_node(Propagation::new("d2", Length::new::<millimeter>(200.0))?)?;
    let window = group.add_node(IdealFilter::new("window", &window_filter)?)?;
    let d3 = group.add_node(Propagation::new("d3", Length::new::<millimeter>(200.0))?)?;
    let hhts_t1_cm = group.add_node(BeamSplitter::new("HHTS_T1_CM", &dichroic_mirror)?)?;

    let d4 = group.add_node(Propagation::new("d4", Length::new::<millimeter>(100.0))?)?;
    let beam_dump = group.add_node(EnergyMeter::new("Beamdump", Metertype::IdealEnergyMeter))?;

    let d5 = group.add_node(Propagation::new("d5", Length::new::<millimeter>(1000.0))?)?;
    let hhts_t1_pm = group.add_node(BeamSplitter::new("HHTS_T1_PM", &double_mirror)?)?;
    let d6 = group.add_node(Propagation::new("d6", Length::new::<millimeter>(100.0))?)?;

    group.connect_nodes(d1, "rear", mm15, "input1")?;
    group.connect_nodes(mm15, "out1_trans1_refl2", d2, "front")?;
    group.connect_nodes(d2, "rear", window, "front")?;
    group.connect_nodes(window, "rear", d3, "front")?;
    group.connect_nodes(d3, "rear", hhts_t1_cm, "input1")?;
    group.connect_nodes(hhts_t1_cm, "out1_trans1_refl2", d4, "front")?;
    group.connect_nodes(d4, "rear", beam_dump, "in1")?;
    group.connect_nodes(hhts_t1_cm, "out2_trans2_refl1", d5, "front")?;
    group.connect_nodes(d5, "rear", hhts_t1_pm, "input1")?;
    group.connect_nodes(hhts_t1_pm, "out2_trans2_refl1", d6, "front")?;

    group.map_input_port(d1, "front", "input")?;
    group.map_output_port(d6, "rear", "output")?;
    Ok(group)
}
