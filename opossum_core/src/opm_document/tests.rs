use super::*;
use crate::{
    analyzers::{AnalyzerType, RayTraceConfig, energy::EnergyConfig},
    core_optics::node_attr::HasNodeAttr,
    gain::{ConstGain, GainModel},
    nodes::Lens,
};

#[test]
fn new() {
    let mut scenery = NodeGroup::default();
    scenery.node_attr_mut().set_name("MyTest");
    let document = OpmDocument::new(scenery);
    assert_eq!(document.scenery.node_attr().name(), "MyTest");
    assert!(document.analyzers.is_empty());
}

#[test]
fn default() {
    let document = OpmDocument::default();
    assert_eq!(document.opm_file_version, env!("OPM_FILE_VERSION"));
    assert!(document.analyzers.is_empty());
    assert!(document.pump_scenarios.is_empty());
}

#[test]
fn pump_scenario_crud() {
    let mut document = OpmDocument::default();
    let id = document.add_pump_scenario("full power");
    assert_eq!(document.pump_scenarios().len(), 1);
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("full power")
    );
    assert!(document.pump_scenario(Uuid::new_v4()).is_none());

    document
        .pump_scenario_mut(id)
        .expect("the scenario just added must be there")
        .set_name("half power");
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("half power")
    );

    let removed = document.remove_pump_scenario(id);
    assert_eq!(removed.as_ref().map(PumpScenario::name), Some("half power"));
    assert!(document.pump_scenarios().is_empty());
    assert!(document.remove_pump_scenario(id).is_none());

    // Restoring a scenario keeps its identity
    document.insert_pump_scenario(id, removed.expect("the scenario was removed above"));
    assert_eq!(
        document.pump_scenario(id).map(PumpScenario::name),
        Some("half power")
    );
}

#[test]
fn pruning_follows_deleted_nodes_in_every_scenario() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let deleted_id = document.scenery_mut().add_node(Lens::default())?;
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    for name in ["full power", "half power"] {
        let scenario_id = document.add_pump_scenario(name);
        let scenario = document
            .pump_scenario_mut(scenario_id)
            .expect("the scenario just added must be there");
        scenario.set_gain_model(lens_id, gain);
        scenario.set_gain_model(deleted_id, gain);
    }
    document.scenery_mut().delete_node(deleted_id)?;
    document.prune_pump_scenarios();

    for scenario in document.pump_scenarios().values() {
        assert_eq!(scenario.gain_model(lens_id), gain);
        assert_eq!(scenario.gain_model(deleted_id), GainModel::None);
    }
    Ok(())
}

#[test]
fn amplifier_node_candidacy_roundtrip() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    assert!(!document.is_amplifier_node(lens_id));
    assert!(document.amplifier_nodes().is_empty());

    document.set_is_amplifier_node(lens_id, true);
    assert!(document.is_amplifier_node(lens_id));
    assert_eq!(document.amplifier_nodes(), &HashSet::from([lens_id]));

    document.set_is_amplifier_node(lens_id, false);
    assert!(!document.is_amplifier_node(lens_id));
    assert!(document.amplifier_nodes().is_empty());
    Ok(())
}

#[test]
fn unmarking_a_candidate_wipes_its_gain_model_in_every_scenario() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    document.set_is_amplifier_node(lens_id, true);
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    let full_power = document.add_pump_scenario("full power");
    let half_power = document.add_pump_scenario("half power");
    document
        .pump_scenario_mut(full_power)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);
    document
        .pump_scenario_mut(half_power)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);

    document.set_is_amplifier_node(lens_id, false);

    for scenario_id in [full_power, half_power] {
        assert_eq!(
            document
                .pump_scenario(scenario_id)
                .map(|scenario| scenario.gain_model(lens_id)),
            Some(GainModel::None)
        );
    }
    Ok(())
}

#[test]
fn set_amplifier_nodes_replaces_the_whole_set_without_touching_scenarios() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let scenario_id = document.add_pump_scenario("full power");
    let gain = GainModel::Const(ConstGain::new(2.0)?);
    document
        .pump_scenario_mut(scenario_id)
        .expect("the scenario just added must be there")
        .set_gain_model(lens_id, gain);

    document.set_amplifier_nodes(HashSet::new());
    assert!(document.amplifier_nodes().is_empty());
    assert_eq!(
        document
            .pump_scenario(scenario_id)
            .map(|scenario| scenario.gain_model(lens_id)),
        Some(gain),
        "a whole-set replace must not wipe any scenario's gain model"
    );

    document.set_amplifier_nodes(HashSet::from([lens_id]));
    assert_eq!(document.amplifier_nodes(), &HashSet::from([lens_id]));
    Ok(())
}

#[test]
fn prune_amplifier_nodes_drops_deleted_node_entries() -> OpmResult<()> {
    let mut document = OpmDocument::default();
    let lens_id = document.scenery_mut().add_node(Lens::default())?;
    let deleted_id = document.scenery_mut().add_node(Lens::default())?;
    document.set_is_amplifier_node(lens_id, true);
    document.set_is_amplifier_node(deleted_id, true);
    document.scenery_mut().delete_node(deleted_id)?;

    document.prune_amplifier_nodes();

    assert!(document.is_amplifier_node(lens_id));
    assert!(!document.is_amplifier_node(deleted_id));
    Ok(())
}

#[test]
fn add_analyzer() {
    let mut document = OpmDocument::default();
    assert!(document.analyzers.is_empty());
    document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    assert_eq!(document.analyzers.len(), 1);
}

#[test]
fn add_analyzer_with_position() {
    let mut document = OpmDocument::default();
    let uuid =
        document.add_analyzer_with_position(AnalyzerType::Energy(EnergyConfig::default()), None);
    assert!(!uuid.is_nil());
}

#[test]
fn analyzers() {
    let mut document = OpmDocument::default();
    document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    document.add_analyzer(AnalyzerType::RayTrace(RayTraceConfig::default()));
    assert_eq!(document.analyzers().len(), 2);
}

#[test]
fn analyzer() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.analyzer(uuid1).is_ok());
    assert!(document.analyzer(uuid2).is_ok());
    assert!(document.analyzer(Uuid::nil()).is_err());
}

#[test]
fn analyzer_mut() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.analyzer_mut(uuid1).is_some());
    assert!(document.analyzer_mut(uuid2).is_some());
    assert!(document.analyzer_mut(Uuid::nil()).is_none());
}

#[test]
fn remove_analyzer() {
    let mut document = OpmDocument::default();
    let uuid1 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));
    let uuid2 = document.add_analyzer(AnalyzerType::Energy(EnergyConfig::default()));

    assert!(document.remove_analyzer(uuid1).is_ok());
    assert_eq!(document.analyzers.len(), 1);
    assert!(document.remove_analyzer(Uuid::nil()).is_err());
    assert!(document.remove_analyzer(uuid2).is_ok());
    assert!(document.analyzers.is_empty());
}
