use super::BeamSplitter;
use crate::{
    analyzers::energy::AnalysisEnergy, error::OpmResult, light_result::LightResult,
    optic_node::OpticNode,
};

impl AnalysisEnergy for BeamSplitter {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let (input_port1, input_port2) = if self.inverted() {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input_1", "input_2")
        };
        let in1 = incoming_data.get(input_port1);
        let in2 = incoming_data.get(input_port2);
        let (out1_data, out2_data) = self.analyze_energy(in1, in2)?;
        if let Some(out1_data) = out1_data
            && let Some(out2_data) = out2_data
        {
            let (target1, target2) = if self.inverted() {
                ("input_1", "input_2")
            } else {
                ("out1_trans1_refl2", "out2_trans2_refl1")
            };
            Ok(LightResult::from([
                (target1.into(), out1_data),
                (target2.into(), out2_data),
            ]))
        } else {
            Ok(LightResult::default())
        }
    }
}

#[cfg(test)]
mod test {
    use approx::{AbsDiffEq, assert_abs_diff_eq};

    use crate::{
        analyzers::energy::AnalysisEnergy,
        joule,
        light_result::LightResult,
        lightdata::LightData,
        nanometer,
        nodes::{BeamSplitter, SplittingConfigBuilder},
        optic_node::OpticNode,
        prelude::{
            EdgeFilter, EdgeFilterType, EnergyDataBuilder, EnergyLaserLines, LightDataBuilder,
            SpectralFilterBuilder,
        },
        spectrum_helper::create_he_ne_spec,
    };

    #[test]
    fn analyze_empty_input() {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_one_input() {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6)).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "input_1".into(),
            LightData::Energy(create_he_ne_spec(1.0).unwrap()),
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        let result = output.clone().get("out1_trans1_refl2").unwrap().clone();
        let energy = if let LightData::Energy(s) = result {
            s.total_energy()
        } else {
            0.0
        };
        assert_eq!(energy, 0.6);
        let result = output.clone().get("out2_trans2_refl1").unwrap().clone();
        let energy = if let LightData::Energy(s) = result {
            s.total_energy()
        } else {
            0.0
        };
        assert_eq!(energy, 0.4);
    }
    #[test]
    fn analyze_two_input_fixed_ratio() {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6)).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "input_1".into(),
            LightData::Energy(create_he_ne_spec(1.0).unwrap()),
        );
        input.insert(
            "input_2".into(),
            LightData::Energy(create_he_ne_spec(0.5).unwrap()),
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        let energy_output1 = if let LightData::Energy(s) =
            output.clone().get("out1_trans1_refl2").unwrap().clone()
        {
            s.total_energy()
        } else {
            0.0
        };
        assert!(energy_output1.abs_diff_eq(&0.8, f64::EPSILON));
        let energy_output2 = if let LightData::Energy(s) =
            output.clone().get("out2_trans2_refl1").unwrap().clone()
        {
            s.total_energy()
        } else {
            0.0
        };
        assert!(energy_output2.abs_diff_eq(&0.7, f64::EPSILON));
    }
    #[test]
    fn analyze_one_input_longpass() {
        let edge_filter = EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(1000.0),
            0.0..1.0,
            Some(nanometer!(0.4)),
            nanometer!(900.0)..nanometer!(1100.0),
            nanometer!(0.2),
        )
        .unwrap();
        let longpass = SpectralFilterBuilder::EdgeFilter(edge_filter);
        let mut node =
            BeamSplitter::new("test", &SplittingConfigBuilder::Spectrum(longpass)).unwrap();
        let light_data = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(1054.0), joule!(100.0))], nanometer!(5.0))
                .unwrap(),
        ));
        let mut input = LightResult::default();
        input.insert("input_1".into(), light_data.build().unwrap());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        let LightData::Energy(s1) = output.clone().get("out1_trans1_refl2").unwrap().clone() else {
            panic!();
        };
        let LightData::Energy(s2) = output.clone().get("out2_trans2_refl1").unwrap().clone() else {
            panic!();
        };
        assert_abs_diff_eq!(s1.total_energy(), 100.0, epsilon = 0.0001);
        assert_abs_diff_eq!(s2.total_energy(), 0.0);
    }
    #[test]
    fn analyze_inverse() {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6)).unwrap();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "out1_trans1_refl2".into(),
            LightData::Energy(create_he_ne_spec(1.0).unwrap()),
        );
        input.insert(
            "out2_trans2_refl1".into(),
            LightData::Energy(create_he_ne_spec(0.5).unwrap()),
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        let energy_output1 =
            if let LightData::Energy(s) = output.clone().get("input_1").unwrap().clone() {
                s.total_energy()
            } else {
                0.0
            };

        let energy_output2 =
            if let LightData::Energy(s) = output.clone().get("input_2").unwrap().clone() {
                s.total_energy()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output1, &0.8);
        assert_abs_diff_eq!(energy_output2, &0.7);
    }
}
