use opm_macros_lib::OpmNode;

use crate::{
    analyzers::{
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        raytrace::AnalysisRayTrace,
    },
    core_optics::NodeAttr,
    error::{OpmResult, OpossumError},
    light::Rays,
    light_result::{
        LightRays, LightResult, light_rays_to_light_result, light_result_to_light_rays,
    },
    lightdata::LightData,
    nodes::{FilterType, NodeRegistration},
    prelude::{FilterTypeBuilder, GhostFocusConfig, OpticNode, PortType, Proptype, RayTraceConfig},
    properties::validator::Validator,
};

inventory::submit! {
    NodeRegistration::new::<IdealFilter>("ideal filter", "ideal filter")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("darkgray")]
/// An ideal filter with given transmission or optical density.
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `filter type`
pub struct IdealFilter {
    node_attr: NodeAttr,
}
unsafe impl Send for IdealFilter {}

impl Default for IdealFilter {
    /// Create an ideal filter node with a transmission of 100%.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ideal filter");
        node_attr
            .create_property_with_validator(
                "filter type builder",
                "used filter algorithm",
                Validator::NumericInRange { min: 0., max: 1. },
                FilterTypeBuilder::Constant(1.0).into(),
            )
            .unwrap();
        let mut idf = Self { node_attr };
        idf.update_surfaces().unwrap();
        idf
    }
}
impl IdealFilter {
    /// Creates a new [`IdealFilter`] with a given [`FilterType`].
    ///
    /// # Errors
    ///
    /// This function will return an [`OpossumError::Other`] if the filter type is
    /// [`FilterType::Constant`] and the transmission factor is outside the interval [0.0; 1.0].
    pub fn new(name: &str, filter_type_builder: &FilterTypeBuilder) -> OpmResult<Self> {
        let mut filter = Self::default();
        filter
            .node_attr
            .set_property("filter type builder", filter_type_builder.clone().into())?;
        filter.node_attr.set_name(name);
        Ok(filter)
    }
    /// Returns the filter type of this [`IdealFilter`].
    ///
    /// # Errors
    /// Errors if the wrong data type is stored in the filter-type properties
    pub fn filter_type(&self) -> OpmResult<FilterType> {
        if let Proptype::FilterTypeBuilder(filter_type_builder) =
            self.node_attr.get_property("filter type builder")?
        {
            filter_type_builder.build()
        } else {
            Err(OpossumError::Properties(
                "Property: `filter type builder` not found".into(),
            ))
        }
    }
    /// Sets a constant transmission value for this [`IdealFilter`].
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if a transmission factor > 1.0 is given (This would be an amplifiying filter :-) ).
    pub fn set_transmission(&mut self, transmission: f64) -> OpmResult<()> {
        if (0.0..=1.0).contains(&transmission) {
            self.node_attr.set_property(
                "filter type builder",
                FilterTypeBuilder::Constant(transmission).into(),
            )?;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "attenuation must be in interval [0.0; 1.0]".into(),
            ))
        }
    }
    /// Sets the transmission of this [`IdealFilter`] expressed as optical density.
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if an optical density < 0.0 was given.
    pub fn set_optical_density(&mut self, density: f64) -> OpmResult<()> {
        if density >= 0.0 {
            self.node_attr.set_property(
                "filter type builder",
                FilterTypeBuilder::Constant(f64::powf(10.0, -density)).into(),
            )?;
            Ok(())
        } else {
            Err(OpossumError::Other("optical densitiy must be >=0".into()))
        }
    }
    /// Returns the transmission factor of this [`IdealFilter`] expressed as optical density for the [`FilterType::Constant`].
    ///
    /// This functions `None` if the filter type is not [`FilterType::Constant`].
    #[must_use]
    pub fn optical_density(&self) -> Option<f64> {
        self.filter_type()
            .map_or(None, |filter_type| match filter_type {
                FilterType::Constant(t) => Some(-f64::log10(t)),
                FilterType::Spectrum(_) => None,
            })
    }
}
impl OpticNode for IdealFilter {
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.update_flat_single_surfaces()
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
}
impl AnalysisGhostFocus for IdealFilter {
    fn analyze(
        &mut self,
        incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let filter_type = self.filter_type()?;
        let incoming_result = light_rays_to_light_result(incoming_data);
        let output =
            self.unified_analyze_single_surface_node(incoming_result, config, "input_1", None)?;
        let light_rays = light_result_to_light_rays(output)?;
        let out_port = &self.ports().names(&PortType::Output)[0];
        if let Some(rays_bundles) = light_rays.clone().get_mut(out_port) {
            for rays in rays_bundles {
                rays.filter_energy(&filter_type)?;
            }
            Ok(light_rays)
        } else {
            Err(OpossumError::Analysis("filtering of rays failed".into()))
        }
    }
}
impl AnalysisEnergy for IdealFilter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        _config: &EnergyConfig,
    ) -> OpmResult<LightResult> {
        let filter_type = self.filter_type()?;
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(input) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Energy(s) = input {
            let mut new_spectrum = s.clone();
            new_spectrum.filter_with_type(&filter_type)?;
            let light_data = LightData::Energy(new_spectrum);
            Ok(LightResult::from([(out_port.into(), light_data)]))
        } else {
            Err(OpossumError::Analysis("expected energy light data".into()))
        }
    }
}
impl AnalysisRayTrace for IdealFilter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        config: &RayTraceConfig,
    ) -> OpmResult<LightResult> {
        let filter_type = self.filter_type()?;

        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(input) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        let LightData::Geometric(r) = input else {
            return Err(OpossumError::Analysis(
                "expected geometric light data".into(),
            ));
        };
        let mut rays = r.clone();
        let iso = self.effective_surface_iso(in_port)?;
        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        };
        let refraction_intended = true;
        rays.refract_on_surface(
            surf,
            None,
            refraction_intended,
            config.missed_surface_strategy(),
        )?;
        rays.filter_energy(&filter_type)?;
        match self.ports().aperture(&PortType::Input, in_port) {
            Some(aperture) => {
                rays.apodize(aperture, &iso)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            }
        }
        match self.ports().aperture(&PortType::Output, out_port) {
            Some(aperture) => {
                rays.apodize(aperture, &iso)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("output aperture not found".into()));
            }
        }
        let light_data = LightData::Geometric(rays);
        Ok(LightResult::from([(out_port.into(), light_data)]))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        distributions::position::Hexapolar,
        joule,
        light::spectrum_helper::create_he_ne_spec,
        millimeter, nanometer,
        nodes::test_helper::test_helper::{
            test_analyze_empty, test_analyze_wrong_data_type, test_inverted,
        },
        prelude::{BandFilter, Isometry},
    };
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;
    #[test]
    fn default() {
        let mut node = IdealFilter::default();
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(1.0));
        assert_eq!(node.name(), "ideal filter");
        assert_eq!(node.node_type(), "ideal filter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "darkgray");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        assert!(IdealFilter::new("test", &FilterTypeBuilder::Constant(1.1)).is_err());
        assert!(IdealFilter::new("test", &FilterTypeBuilder::Constant(-0.1)).is_err());
        let node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.8)).unwrap();
        assert_eq!(node.name(), "test");
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.8));
    }
    #[test]
    fn set_transmission() {
        let mut node = IdealFilter::default();
        assert!(node.set_transmission(-0.1).is_err());
        assert!(node.set_transmission(1.1).is_err());
        assert!(node.set_transmission(0.5).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.5));
    }
    #[test]
    fn optical_density() {
        let mut node = IdealFilter::default();
        assert_eq!(node.optical_density(), Some(0.0));
        node.set_transmission(0.1).unwrap();
        assert_eq!(node.optical_density(), Some(1.0));
        node.set_transmission(0.01).unwrap();
        assert_eq!(node.optical_density(), Some(2.0));
        let node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Spectrum(BandFilter::default().into()),
        )
        .unwrap();
        assert_eq!(node.optical_density(), None);
    }
    #[test]
    fn set_optical_density() {
        let mut node = IdealFilter::default();
        assert!(node.set_optical_density(-1.0).is_err());
        assert!(node.set_optical_density(1.0).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.1));
        assert!(node.set_optical_density(f64::NAN).is_err());
        assert!(node.set_optical_density(f64::INFINITY).is_ok());
        assert_eq!(node.filter_type().unwrap(), FilterType::Constant(0.0));
    }
    #[test]
    fn inverted() {
        test_inverted::<IdealFilter>()
    }
    #[test]
    fn ports() {
        let node = IdealFilter::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = IdealFilter::default();
        node.set_inverted(true).unwrap();
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<IdealFilter>()
    }
    #[test]
    fn analyze_wrong() {
        let mut node = IdealFilter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<IdealFilter>("input_1");
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.5)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisRayTrace::analyze(&mut node, input.clone(), &RayTraceConfig::default())
                .is_err()
        );
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5).unwrap());
        assert_eq!(*output, expected_output_light);
    }
    #[test]
    fn analyzer_geometric_fixed() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.3)).unwrap();
        node.set_isometry(Isometry::identity()).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(
            Rays::new_uniform_collimated(
                nanometer!(1054.0),
                joule!(1.0),
                &Hexapolar::new(millimeter!(5.0), 1).unwrap(),
            )
            .unwrap(),
        );
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisEnergy::analyze(&mut node, input.clone(), &EnergyConfig::default()).is_err()
        );
        let output =
            AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default()).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        if let LightData::Geometric(output) = output.clone().unwrap() {
            assert_abs_diff_eq!(output.total_energy().get::<joule>(), 0.3);
        } else {
            panic!("wrong data LightData format")
        }
    }
    #[test]
    fn analyze_inverse() {
        let mut node = IdealFilter::new("test", &FilterTypeBuilder::Constant(0.5)).unwrap();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0).unwrap());
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default()).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5).unwrap());
        assert_eq!(*output, expected_output_light);
    }
}
