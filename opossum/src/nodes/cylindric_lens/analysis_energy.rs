use crate::{
    analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult,
    optic_node::OpticNode, optic_ports::PortType,
};

use super::CylindricLens;

impl AnalysisEnergy for CylindricLens {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(data) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        Ok(LightResult::from([(out_port.into(), data.clone())]))
    }
}
#[cfg(test)]
mod test {
    use crate::{
        analyzers::energy::AnalysisEnergy,
        light_result::LightResult,
        lightdata::{DataEnergy, LightData},
        nodes::CylindricLens,
        spectrum_helper::create_he_ne_spec,
    };
    #[test]
    fn analyze_empty() {
        let mut node = CylindricLens::default();
        let output = node.analyze(LightResult::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_ok() {
        let mut node = CylindricLens::default();
        let mut input = LightResult::default();
        input.insert(
            "input_1".to_string(),
            LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            }),
        );
        let output = node.analyze(input).unwrap();
        assert!(output.contains_key("output_1"));
    }
}
