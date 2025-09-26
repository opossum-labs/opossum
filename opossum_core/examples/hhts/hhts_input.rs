use opossum_core::prelude::*;
use std::path::Path;

pub fn hhts_input() -> OpmResult<NodeGroup> {
    let dichroic_mirror = SplittingConfigBuilder::Spectrum(SpectralFilterBuilder::FromFile(
        Path::new("opossum/examples/hhts/MM15_Transmission.csv").to_path_buf(),
    ));
    let window_filter = FilterTypeBuilder::Spectrum(SpectralFilterBuilder::FromFile(
        Path::new("opossum/examples/hhts/HHTS_W1_Transmission.csv").to_path_buf(),
    ));
    let double_mirror = SplittingConfigBuilder::Spectrum(SpectralFilterBuilder::FromFile(
        Path::new("opossum/examples/hhts/HHTS_T1_PM_Transmission.csv").to_path_buf(),
    ));
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
