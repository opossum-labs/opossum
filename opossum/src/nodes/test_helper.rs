#[cfg(test)]
pub mod test_helper {
    use crate::{
        analyzer::AnalyzerType,
        aperture::CircleConfig,
        joule,
        lightdata::LightData,
        millimeter, nanometer,
        optical::{LightResult, Optical},
        position_distributions::Hexapolar,
        rays::Rays,
        utils::geom_transformation::Isometry,
    };
    pub fn test_inverted<T: Default + Optical>() {
        let mut node = T::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    pub fn test_analyze_empty<T: Default + Optical>() {
        let mut node = T::default();
        let input = LightResult::default();
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.is_empty());
    }
    pub fn test_analyze_apodization_warning<T: Default + Optical>() {
        testing_logger::setup();
        let mut node = T::default();
        node.set_isometry(Isometry::identity());
        let config = CircleConfig::new(millimeter!(1.0), millimeter!(0.0, 0.0)).unwrap();
        node.set_input_aperture("in1", &crate::aperture::Aperture::BinaryCircle(config))
            .unwrap();
        let mut input = LightResult::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let input_light = LightData::Geometric(rays);
        input.insert("in1".into(), input_light.clone());
        node.analyze(input, &AnalyzerType::Energy).unwrap();
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            let msg=format!("Rays have been apodized at input aperture of {} <{}>. Results might not be accurate.", 
            node.node_attr().name(),
            node.node_attr().node_type());
            assert_eq!(captured_logs[0].body, msg);
        });
    }
}
