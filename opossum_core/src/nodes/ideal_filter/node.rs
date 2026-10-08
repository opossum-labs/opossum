use opm_macros_lib::OpmNode;
use uom::si::f64::Ratio;

use crate::{
    analyzers::{
        energy::{AnalysisEnergy, EnergyConfig},
        ghostfocus::AnalysisGhostFocus,
        propagation_strategy::PropagationStrategy,
        raytrace::AnalysisRayTrace,
    },
    core_optics::{NodeAttr, NodeAttrExt, OpticNodeExt},
    error::{OpmResult, OpossumError},
    geometry::Geometry,
    light::{LightData, LightRays, LightResult, Rays},
    nodes::{
        FilterType, NodeRegistration, create_surface_properties,
        ideal_filter::filter_types::FilterConst,
    },
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
///     - `input_1`
///   - Outputs
///     - `output_1`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `filter type builder`: Config data for the filter type. See [`FilterTypeBuilder`] for details.
///   - `clear aperture`: the extent of the filter; a ray beyond it misses the filter, is not
///     filtered and follows the analyzer's missed surface strategy
pub struct IdealFilter {
    node_attr: NodeAttr,
}

impl Default for IdealFilter {
    /// Create an ideal filter node with a transmission of 100%.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("ideal filter");
        node_attr
            .create_property_with_validator(
                "filter type builder",
                "used filter algorithm",
                Validator::NumericInRange { min: 0., max: 1. },
                FilterTypeBuilder::default().into(),
            )
            .unwrap();
        create_surface_properties(&mut node_attr).unwrap();
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
    pub fn set_transmission(&mut self, transmission: Ratio) -> OpmResult<()> {
        let transmission_constant = FilterConst::new(transmission)?;
        self.node_attr.set_property(
            "filter type builder",
            FilterTypeBuilder::Constant(transmission_constant).into(),
        )?;
        Ok(())
    }
    /// Sets the transmission of this [`IdealFilter`] expressed as optical density.
    ///
    /// This implicitly sets the filter type to [`FilterType::Constant`].
    /// # Errors
    ///
    /// This function will return an error if an optical density < 0.0 was given.
    pub fn set_optical_density(&mut self, density: f64) -> OpmResult<()> {
        let transmission_constant = FilterConst::new((f64::powf(10.0, -density)).into())?;
        self.node_attr.set_property(
            "filter type builder",
            FilterTypeBuilder::Constant(transmission_constant).into(),
        )?;
        Ok(())
    }
    /// Returns the transmission factor of this [`IdealFilter`] expressed as optical density for the [`FilterType::Constant`].
    ///
    /// This functions `None` if the filter type is not [`FilterType::Constant`].
    #[must_use]
    pub fn optical_density(&self) -> Option<f64> {
        self.filter_type()
            .map_or(None, |filter_type| match filter_type {
                FilterType::Constant(t) => Some(-f64::log10(t.transmission().value)),
                FilterType::Spectrum(_) => None,
            })
    }
}
impl OpticNode for IdealFilter {
    fn geometry(&self) -> OpmResult<Option<Geometry>> {
        Ok(Some(Geometry::plane(self.clear_aperture()?)))
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        self.install_geometry(&["input_1"], &["output_1"])
    }
}
impl AnalysisGhostFocus for IdealFilter {
    fn analyze(
        &mut self,
        mut incoming_data: LightRays,
        config: &GhostFocusConfig,
        _ray_collection: &mut Vec<Rays>,
        _bounce_lvl: usize,
    ) -> OpmResult<LightRays> {
        let filter_type = self.filter_type()?;
        let in_port = &self.ports().names(&PortType::Input)[0];
        let out_port = &self.ports().names(&PortType::Output)[0];
        let Some(mut rays_bundle) = incoming_data.remove(in_port) else {
            return Err(OpossumError::Analysis("filtering of rays failed".into()));
        };
        let hits = self.pass_through_surface_generic(
            "input_1",
            None,
            &mut rays_bundle,
            config,
            false,
            true,
        )?;
        for (rays, hits) in rays_bundle.iter_mut().zip(&hits) {
            rays.filter_energy_hits(&filter_type, hits)?;
        }
        Ok(LightRays::from([(out_port.clone(), rays_bundle)]))
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
        let rays_before = rays.nr_of_rays(true);
        let iso = self.effective_surface_iso(in_port)?;
        let Some(surf) = self.get_optic_surface_mut(in_port) else {
            return Err(OpossumError::Analysis("no surface found. Aborting".into()));
        };
        let refraction_intended = true;
        let (_, hits) = rays.refract_on_surface(
            surf,
            None,
            refraction_intended,
            config.missed_surface_strategy(),
        )?;
        // A ray that ran past the filter is neither filtered nor masked by its port apertures.
        rays.filter_energy_hits(&filter_type, &hits)?;
        match self.ports().aperture(&PortType::Input, in_port) {
            // Only the components' rims decide where the optical axis runs, not its energy.
            Some(_) if config.is_positioning_run() => {}
            Some(aperture) => {
                rays.apodize_hits(aperture, &iso, &hits)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("input aperture not found".into()));
            }
        }
        match self.ports().aperture(&PortType::Output, out_port) {
            Some(_) if config.is_positioning_run() => {}
            Some(aperture) => {
                rays.apodize_hits(aperture, &iso, &hits)?;
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
            _ => {
                return Err(OpossumError::OpticPort("output aperture not found".into()));
            }
        }
        self.warn_about_lost_rays(rays_before, rays.nr_of_rays(true));
        let light_data = LightData::Geometric(rays);
        Ok(LightResult::from([(out_port.into(), light_data)]))
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::propagation_strategy::MissedSurfaceStrategy,
        apertures::{Aperture, ApertureType},
        core_optics::node_attr::NodePositioning,
        distributions::position::Hexapolar,
        joule,
        light::spectrum_helper::create_he_ne_spec,
        millimeter, nanometer,
        nodes::test_helper::helper::{
            assert_valid_energies, placed_at_origin, rays_along_z_from, test_analyze_empty,
            test_analyze_wrong_data_type, test_clear_aperture_absent_in_file, test_inverted,
            test_rays_beyond_clear_aperture_are_lost,
        },
        percent,
        prelude::{BandFilter, Isometry},
    };
    use approx::assert_abs_diff_eq;
    use uom::si::energy::joule;
    #[test]
    fn default() -> OpmResult<()> {
        let node = IdealFilter::default();
        assert_eq!(
            node.filter_type()?,
            FilterType::Constant(FilterConst::default())
        );
        assert_eq!(node.name(), "ideal filter");
        assert_eq!(node.node_type(), "ideal filter");
        assert!(!node.inverted());
        assert_eq!(node.node_color(), "darkgray");
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(80.0))?),
        )?;
        assert_eq!(node.name(), "test");
        assert_eq!(
            node.filter_type()?,
            FilterType::Constant(FilterConst::new(percent!(80.0))?)
        );
        Ok(())
    }
    #[test]
    fn set_transmission() -> OpmResult<()> {
        let mut node = IdealFilter::default();
        assert!(node.set_transmission(percent!(-1.0)).is_err());
        assert!(node.set_transmission(percent!(100.1)).is_err());
        assert!(node.set_transmission(percent!(50.0)).is_ok());
        assert_eq!(
            node.filter_type()?,
            FilterType::Constant(FilterConst::new(percent!(50.0))?)
        );
        assert!(node.set_transmission(percent!(0.0)).is_ok());
        Ok(())
    }
    #[test]
    fn optical_density() -> OpmResult<()> {
        let mut node = IdealFilter::default();
        assert_eq!(node.optical_density(), Some(0.0));
        node.set_transmission(percent!(10.0))?;
        assert_eq!(node.optical_density(), Some(1.0));
        node.set_transmission(percent!(1.0))?;
        assert_eq!(node.optical_density(), Some(2.0));
        let node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Spectrum(BandFilter::default().into()),
        )?;
        assert_eq!(node.optical_density(), None);
        Ok(())
    }
    #[test]
    fn set_optical_density() -> OpmResult<()> {
        let mut node = IdealFilter::default();
        assert!(node.set_optical_density(-1.0).is_err());
        assert!(node.set_optical_density(1.0).is_ok());
        assert_eq!(
            node.filter_type()?,
            FilterType::Constant(FilterConst::new(percent!(10.0))?)
        );
        assert!(node.set_optical_density(f64::NAN).is_err());
        assert!(node.set_optical_density(f64::INFINITY).is_ok());
        assert_eq!(
            node.filter_type()?,
            FilterType::Constant(FilterConst::new(percent!(0.0))?)
        );
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<IdealFilter>()
    }
    #[test]
    fn ports() {
        let node = IdealFilter::default();
        assert_eq!(node.ports().names(&PortType::Input), vec!["input_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["output_1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut node = IdealFilter::default();
        node.set_inverted(true)?;
        assert_eq!(node.ports().names(&PortType::Input), vec!["output_1"]);
        assert_eq!(node.ports().names(&PortType::Output), vec!["input_1"]);
        Ok(())
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<IdealFilter>()
    }
    #[test]
    fn analyze_wrong() -> OpmResult<()> {
        let mut node = IdealFilter::default();
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.is_empty());
        Ok(())
    }
    #[test]
    fn analyze_geometric_wrong_data_type() -> OpmResult<()> {
        test_analyze_wrong_data_type::<IdealFilter>("input_1")
    }
    #[test]
    fn analyze_energy_ok() -> OpmResult<()> {
        let mut node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(50.0))?),
        )?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisRayTrace::analyze(&mut node, input.clone(), &RayTraceConfig::default())
                .is_err()
        );
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        let output = output.unwrap();
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5)?);
        assert_eq!(*output, expected_output_light);
        Ok(())
    }
    #[test]
    fn analyzer_geometric_fixed() -> OpmResult<()> {
        let mut node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(30.0))?),
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let mut input = LightResult::default();
        let input_light = LightData::Geometric(Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), 1)?,
        )?);
        input.insert("input_1".into(), input_light.clone());
        assert!(
            AnalysisEnergy::analyze(&mut node, input.clone(), &EnergyConfig::default()).is_err()
        );
        let output = AnalysisRayTrace::analyze(&mut node, input, &RayTraceConfig::default())?;
        assert!(output.contains_key("output_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("output_1");
        assert!(output.is_some());
        if let LightData::Geometric(output) = output.unwrap() {
            assert_abs_diff_eq!(output.total_energy().get::<joule>(), 0.3);
        } else {
            panic!("wrong data LightData format")
        }
        Ok(())
    }
    /// A ghost focus analysis attenuates the light passing a filter just as a ray trace does.
    #[test]
    fn a_filter_attenuates_ghost_focus_rays() -> OpmResult<()> {
        let mut node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(30.0))?),
        )?;
        node.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let rays = Rays::new_uniform_collimated(
            nanometer!(1054.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), 1)?,
        )?;
        let output = AnalysisGhostFocus::analyze(
            &mut node,
            LightRays::from([("input_1".into(), vec![rays])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let Some(passed) = output.get("output_1").and_then(|bundles| bundles.first()) else {
            panic!("expected a ray bundle at the output port");
        };
        assert_abs_diff_eq!(passed.total_energy().get::<joule>(), 0.3);
        Ok(())
    }
    #[test]
    fn clear_aperture_absent_in_file() -> OpmResult<()> {
        test_clear_aperture_absent_in_file::<IdealFilter>()
    }
    #[test]
    fn rays_beyond_clear_aperture_are_lost() -> OpmResult<()> {
        test_rays_beyond_clear_aperture_are_lost::<IdealFilter>()
    }
    /// A filter of 30 % at the origin.
    fn placed_filter() -> OpmResult<IdealFilter> {
        let mut node = placed_at_origin::<IdealFilter>()?;
        node.set_transmission(percent!(30.0))?;
        Ok(node)
    }
    /// Rays along z towards a filter at the origin, one at each of the given heights in mm.
    fn rays_at(heights: &[f64]) -> OpmResult<Rays> {
        let starts: Vec<_> = heights
            .iter()
            .map(|y| millimeter!(0.0, *y, -10.0))
            .collect();
        rays_along_z_from(&starts, nanometer!(1000.0))
    }
    /// A ray running past a filter in a ghost focus analysis is not filtered; one passing it is.
    #[test]
    fn a_filter_does_not_filter_a_ray_beyond_its_rim() -> OpmResult<()> {
        let output = AnalysisGhostFocus::analyze(
            &mut placed_filter()?,
            LightRays::from([("input_1".into(), vec![rays_at(&[12.4, 12.6])?])]),
            &GhostFocusConfig::default(),
            &mut Vec::new(),
            0,
        )?;
        let Some(passed) = output.get("output_1").and_then(|bundles| bundles.first()) else {
            panic!("expected a ray bundle at the output port");
        };
        assert_valid_energies(
            passed,
            &[0.3, 1.0],
            "the ray beyond the rim keeps its energy",
        );
        Ok(())
    }
    /// A port aperture masks the light passing the filter, never a ray that ran past it: with an
    /// analyzer that lets missed rays run on, a ray beyond the clear aperture keeps all its energy,
    /// while a hole of 5 mm at the input or at the output blocks a ray that hits 8 mm off the axis.
    #[test]
    fn a_port_aperture_does_not_touch_a_ray_that_missed_the_filter() -> OpmResult<()> {
        let hole = Aperture::new_circle(millimeter!(5.0), ApertureType::Hole, None)?;
        let mut config = RayTraceConfig::default();
        config.set_missed_surface_strategy(MissedSurfaceStrategy::Ignore);
        for (port_type, port_name) in [(PortType::Input, "input_1"), (PortType::Output, "output_1")]
        {
            let mut node = placed_filter()?;
            node.set_aperture(&port_type, port_name, &hole)?;
            let output = AnalysisRayTrace::analyze(
                &mut node,
                LightResult::from([(
                    "input_1".into(),
                    LightData::Geometric(rays_at(&[8.0, 12.6])?),
                )]),
                &config,
            )?;
            let Some(LightData::Geometric(passed)) = output.get("output_1") else {
                panic!("expected ray data at the output port");
            };
            assert_valid_energies(passed, &[1.0], &format!("hole at {port_name}"));
        }
        Ok(())
    }
    #[test]
    fn analyze_inverse() -> OpmResult<()> {
        let mut node = IdealFilter::new(
            "test",
            &FilterTypeBuilder::Constant(FilterConst::new(percent!(50.0))?),
        )?;
        node.set_inverted(true)?;
        let mut input = LightResult::default();
        let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
        input.insert("output_1".into(), input_light.clone());
        let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
        assert!(output.contains_key("input_1"));
        assert_eq!(output.len(), 1);
        let output = output.get("input_1");
        let expected_output_light = LightData::Energy(create_he_ne_spec(0.5)?);
        assert_eq!(output, Some(&expected_output_light));
        Ok(())
    }
}
