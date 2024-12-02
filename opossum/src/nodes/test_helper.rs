#[cfg(test)]
pub mod test_helper {
    use crate::{
        analyzers::{energy::AnalysisEnergy, raytrace::AnalysisRayTrace, RayTraceConfig},
        aperture::{Aperture, CircleConfig},
        joule,
        light_result::LightResult,
        lightdata::{DataEnergy, LightData},
        millimeter, nanometer,
        optic_node::OpticNode,
        optic_ports::PortType,
        position_distributions::Hexapolar,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
        utils::{geom_transformation::Isometry, test_helper::test_helper::check_logs},
    };
    pub fn test_inverted<T: Default + OpticNode>() {
        let mut node = T::default();
        node.set_inverted(true).unwrap();
        assert_eq!(node.inverted(), true)
    }
    pub fn test_set_aperture<T: Default + OpticNode>(
        input_port_name: &str,
        output_port_name: &str,
    ) {
        let mut node = T::default();
        let aperture = Aperture::default();
        assert!(node
            .set_aperture(&PortType::Input, input_port_name, &aperture)
            .is_ok());
        assert!(node
            .set_aperture(&PortType::Input, output_port_name, &aperture)
            .is_err());
        assert!(node
            .set_aperture(&PortType::Input, "no port", &aperture)
            .is_err());
        assert!(node
            .set_aperture(&PortType::Output, input_port_name, &aperture)
            .is_err());
        assert!(node
            .set_aperture(&PortType::Output, output_port_name, &aperture)
            .is_ok());
        assert!(node
            .set_aperture(&PortType::Output, "no port", &aperture)
            .is_err());
    }
    pub fn test_analyze_empty<T: Default + AnalysisEnergy>() {
        let mut node = T::default();
        let input = LightResult::default();
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    pub fn test_analyze_wrong_data_type<T: Default + AnalysisRayTrace>(input_port_name: &str) {
        let mut node = T::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        assert!(
            node.ports()
                .names(&PortType::Input)
                .contains(&(input_port_name.into())),
            "wrong input port name used"
        );
        input.insert(input_port_name.into(), input_light.clone());
        assert!(AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).is_err());
    }
    pub fn test_analyze_apodization_warning<T: Default + AnalysisRayTrace>() {
        testing_logger::setup();
        let mut node = T::default();
        node.set_isometry(Isometry::identity()).unwrap();
        let config = CircleConfig::new(millimeter!(1.0), millimeter!(0.0, 0.0)).unwrap();
        node.set_aperture(
            &PortType::Input,
            "input_1",
            &crate::aperture::Aperture::BinaryCircle(config),
        )
        .unwrap();
        let mut input = LightResult::default();
        let rays = Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(10.0), 3).unwrap(),
        )
        .unwrap();
        let input_light = LightData::Geometric(rays);
        input.insert("input_1".into(), input_light.clone());
        AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        let msg=format!("Rays have been apodized at input aperture of '{}' ({}). Results might not be accurate.", 
            node.node_attr().name(),
            node.node_attr().node_type());
        check_logs(log::Level::Warn, vec![&msg]);
    }
    pub fn test_analyze_geometric_no_isometry<T: Default + AnalysisRayTrace>(
        input_port_name: &str,
    ) {
        let mut node = T::default();
        assert!(
            node.ports()
                .names(&PortType::Input)
                .contains(&(input_port_name.into())),
            "wrong input port name used"
        );
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::default());
        input.insert(input_port_name.into(), input_light.clone());
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default());
        assert!(output.is_err());
    }
}
