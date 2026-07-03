use super::BeamSplitter;
use crate::{
    analyzers::energy::{AnalysisEnergy, EnergyConfig},
    core_optics::NodeAttrExt,
    error::OpmResult,
    light::LightResult,
};

impl AnalysisEnergy for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
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
        analyzers::energy::{AnalysisEnergy, EnergyConfig},
        core_optics::OpticNode,
        error::OpmResult,
        joule,
        light::{LightData, LightResult, spectrum_helper::create_he_ne_spec},
        nanometer,
        nodes::{BeamSplitter, SplittingConfigBuilder},
        prelude::{
            EdgeFilter, EdgeFilterType, EnergyDataBuilder, EnergyLaserLines, LightDataBuilder,
            SpectralFilterBuilder,
        },
    };

    #[test]
    fn analyze_empty_input() -> OpmResult<()> {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_one_input() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Energy(create_he_ne_spec(1.0)?));
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
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
        Ok(())
    }
    #[test]
    fn analyze_two_input_fixed_ratio() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        let mut input = LightResult::default();
        input.insert("input_1".into(), LightData::Energy(create_he_ne_spec(1.0)?));
        input.insert("input_2".into(), LightData::Energy(create_he_ne_spec(0.5)?));
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
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
        Ok(())
    }
    #[test]
    fn analyze_one_input_longpass() -> OpmResult<()> {
        let edge_filter = EdgeFilter::new(
            EdgeFilterType::LongPass,
            nanometer!(1000.0),
            0.0..1.0,
            Some(nanometer!(0.4)),
            nanometer!(900.0)..nanometer!(1100.0),
            nanometer!(0.2),
        )?;
        let longpass = SpectralFilterBuilder::EdgeFilter(edge_filter);
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::Spectrum(longpass))?;
        let light_data = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
            EnergyLaserLines::new(vec![(nanometer!(1054.0), joule!(100.0))], nanometer!(5.0))?,
        ));
        let mut input = LightResult::default();
        input.insert("input_1".into(), light_data.build()?);
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        let LightData::Energy(s1) = output.clone().get("out1_trans1_refl2").unwrap().clone() else {
            panic!();
        };
        let LightData::Energy(s2) = output.clone().get("out2_trans2_refl1").unwrap().clone() else {
            panic!();
        };
        assert_abs_diff_eq!(s1.total_energy(), 100.0, epsilon = 0.0001);
        assert_abs_diff_eq!(s2.total_energy(), 0.0);
        Ok(())
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        input.insert(
            "out1_trans1_refl2".into(),
            LightData::Energy(create_he_ne_spec(1.0)?),
        );
        input.insert(
            "out2_trans2_refl1".into(),
            LightData::Energy(create_he_ne_spec(0.5)?),
        );
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
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
        Ok(())
    }
}
