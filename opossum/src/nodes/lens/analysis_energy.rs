use crate::{
    analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult,
    optic_node::OpticNode, optic_ports::PortType,
};

use super::Lens;

impl AnalysisEnergy for Lens {
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
    use super::Lens;
    use crate::{
        analyzers::energy::AnalysisEnergy,
        light_result::LightResult,
        lightdata::{DataEnergy, LightData},
        spectrum_helper::create_he_ne_spec,
    };

    #[test]
    fn test_analyze_empty_input() {
        let mut lens = Lens::default();
        let incoming_data = LightResult::default();
        let result = lens.analyze(incoming_data).unwrap();
        assert_eq!(result, LightResult::default());
    }
    #[test]
    fn test_analyze_non_empty_input() {
        let mut lens = Lens::default();
        let incoming_data = LightResult::from([(
            "input_1".into(),
            LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            }),
        )]);
        let result = lens.analyze(incoming_data.clone()).unwrap();
        assert_eq!(result.get("output_1"), incoming_data.get("input_1"));
    }
}
