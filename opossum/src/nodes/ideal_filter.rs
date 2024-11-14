#![warn(missing_docs)]
use super::node_attr::NodeAttr;
use crate::{
    analyzers::{
        energy::AnalysisEnergy, ghostfocus::AnalysisGhostFocus, raytrace::AnalysisRayTrace,
        Analyzable, RayTraceConfig,
    },
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    light_result::LightResult,
    lightdata::LightData,
    optic_node::{Alignable, OpticNode, LIDT},
    optic_ports::PortType,
    properties::Proptype,
    spectrum::Spectrum,
    utils::EnumProxy,
};
use serde::{Deserialize, Serialize};

/// Config data for an [`IdealFilter`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilterType {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    Constant(f64),
    /// filter based on given transmission spectrum.
    Spectrum(Spectrum),
}
#[derive(Debug, Clone)]
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
impl Default for IdealFilter {
    /// Create an ideal filter node with a transmission of 100%.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ideal filter");
        node_attr
            .create_property(
                "filter type",
                "used filter algorithm",
                None,
                EnumProxy::<FilterType> {
                    value: FilterType::Constant(1.0),
                }
                .into(),
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
    pub fn new(name: &str, filter_type: &FilterType) -> OpmResult<Self> {
        if let FilterType::Constant(transmission) = filter_type {
            if !(0.0..=1.0).contains(transmission) {
                return Err(OpossumError::Other(
                    "attenuation must be in interval [0.0; 1.0]".into(),
                ));
            }
        }
        let mut filter = Self::default();
        filter.node_attr.set_property(
            "filter type",
            EnumProxy::<FilterType> {
                value: filter_type.clone(),
            }
            .into(),
        )?;
        filter.node_attr.set_name(name);
        Ok(filter)
    }
    /// Returns the filter type of this [`IdealFilter`].
    ///
    /// # Panics
    /// Panics if the wrong data type is stored in the filter-type properties
    #[must_use]
    pub fn filter_type(&self) -> FilterType {
        if let Proptype::FilterType(filter_type) =
            self.node_attr.get_property("filter type").unwrap()
        {
            filter_type.value.clone()
        } else {
            panic!("wrong data type")
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
                "filter type",
                EnumProxy::<FilterType> {
                    value: FilterType::Constant(transmission),
                }
                .into(),
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
                "filter type",
                EnumProxy::<FilterType> {
                    value: FilterType::Constant(f64::powf(10.0, -1.0 * density)),
                }
                .into(),
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
        match self.filter_type() {
            FilterType::Constant(t) => Some(-1.0 * f64::log10(t)),
            FilterType::Spectrum(_) => None,
        }
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
impl Alignable for IdealFilter {}
impl Dottable for IdealFilter {
    fn node_color(&self) -> &str {
        "darkgray"
    }
}
impl LIDT for IdealFilter {}
impl Analyzable for IdealFilter {}
impl AnalysisGhostFocus for IdealFilter {}
impl AnalysisEnergy for IdealFilter {
    fn analyze(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(input) = incoming_data.get(in_port) else {
            return Ok(LightResult::default());
        };
        if let LightData::Energy(e) = input {
            let mut new_data = e.clone();
            new_data.filter(&self.filter_type())?;
            let light_data = LightData::Energy(new_data);
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
        surf.set_isometry(&iso);
        let refraction_intended = true;
        rays.refract_on_surface(surf, None, refraction_intended)?;
        rays.filter_energy(&self.filter_type())?;
        if let Some(aperture) = self.ports().aperture(&PortType::Input, in_port) {
            rays.apodize(aperture, &iso)?;
            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        } else {
            return Err(OpossumError::OpticPort("input aperture not found".into()));
        };
        if let Some(aperture) = self.ports().aperture(&PortType::Output, out_port) {
            rays.apodize(aperture, &iso)?;
            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
        } else {
            return Err(OpossumError::OpticPort("output aperture not found".into()));
        };

        let light_data = LightData::Geometric(rays);
        Ok(LightResult::from([(out_port.into(), light_data)]))
    }
}

#[cfg(test)]
mod test {
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;

    use crate::{
        analyzers::RayTraceConfig,
        joule,
        lightdata::{DataEnergy, LightData},
        millimeter, nanometer,
        nodes::test_helper::test_helper::*,
        optic_ports::PortType,
        position_distributions::Hexapolar,
        rays::Rays,
        spectrum_helper::create_he_ne_spec,
        utils::geom_transformation::Isometry,
    };

    use super::*;
    #[test]
    fn default() {
        let mut node = IdealFilter::default();
        assert_eq!(node.filter_type(), FilterType::Constant(1.0));
        assert_eq!(node.name(), "ideal filter");
        assert_eq!(node.node_type(), "ideal filter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "darkgray");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        assert!(IdealFilter::new("test", &FilterType::Constant(1.1)).is_err());
        assert!(IdealFilter::new("test", &FilterType::Constant(-0.1)).is_err());
        let node = IdealFilter::new("test", &FilterType::Constant(0.8)).unwrap();
        assert_eq!(node.name(), "test");
        assert_eq!(node.filter_type(), FilterType::Constant(0.8));
    }
    #[test]
    fn set_transmission() {
        let mut node = IdealFilter::default();
        assert!(node.set_transmission(-0.1).is_err());
        assert!(node.set_transmission(1.1).is_err());
        assert!(node.set_transmission(0.5).is_ok());
        assert_eq!(node.filter_type(), FilterType::Constant(0.5));
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
            &FilterType::Spectrum(create_he_ne_spec(1.0).unwrap()),
        )
        .unwrap();
        assert_eq!(node.optical_density(), None);
    }
    #[test]
    fn set_optical_density() {
        let mut node = IdealFilter::default();
        assert!(node.set_optical_density(-1.0).is_err());
        assert!(node.set_optical_density(1.0).is_ok());
        assert_eq!(node.filter_type(), FilterType::Constant(0.1));
        assert!(node.set_optical_density(f64::NAN).is_err());
        assert!(node.set_optical_density(f64::INFINITY).is_ok());
        assert_eq!(node.filter_type(), FilterType::Constant(0.0));
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
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.is_empty());
    }
    #[test]
    fn analyze_geometric_wrong_data_type() {
        test_analyze_wrong_data_type::<IdealFilter>("input_1");
    }
    #[test]
    fn analyze_energy_ok() {
        let mut node = IdealFilter::new("test", &FilterType::Constant(0.5)).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisRayTrace::analyze(&mut node, input.clone(), &RayTraceConfig::default())
                .is_err()
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(0.5).unwrap(),
        });
        assert_eq!(*output, expected_output_light);
    }
    #[test]
    fn analyzer_geometric_fixed() {
        let mut node = IdealFilter::new("test", &FilterType::Constant(0.3)).unwrap();
        node.set_isometry(Isometry::identity());
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
        assert!(AnalysisEnergy::analyze(&mut node, input.clone()).is_err());
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
        let mut node = IdealFilter::new("test", &FilterType::Constant(0.5)).unwrap();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        assert!(output.is_some());
        let output = output.clone().unwrap();
        let expected_output_light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(0.5).unwrap(),
        });
        assert_eq!(*output, expected_output_light);
    }
}
