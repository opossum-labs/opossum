#![warn(missing_docs)]

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

use crate::{
    analyzers::{AnalyzerType, propagation_strategy::MissedSurfaceStrategy},
    coatings::{CoatingConstantR, CoatingType},
    core_optics::{NodeAttr, NodeAttrExt, OpticNode, OpticNodeExt, Planar, PortType, SurfaceKind},
    error::{OpmResult, OpossumError},
    geometry::{Plane, geo_surface::GeoSurfaceRef},
    light::{
        LightData, Rays,
        spectrum::{Spectrum, merge_spectra},
    },
    nodes::{NodeRegistration, create_surface_properties, ideal_filter::SpectralFilterBuilder},
    properties::{Proptype, validator::Validator},
    utils::{default_from_name::DefaultFromName, geom_transformation::Isometry},
};
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};
use strum::{EnumIter, IntoEnumIterator};
use uom::si::{
    f64::{Length, Ratio},
    ratio::ratio,
};

/// Config data builder for a [`BeamSplitter`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
pub enum SplittingConfigBuilder {
    /// a fixed (wavelength-independant) transmission value. Must be between 0.0 and 1.0
    FixedRatio(f64),
    /// splitting based on given transmission spectrum.
    Spectrum(SpectralFilterBuilder),
}

impl SplittingConfigBuilder {
    /// Constructs a [`SplittingConfig`] object from the builder.
    /// # Errors
    /// Returns an error if the creation of a spectrum fails.
    pub fn build(&self) -> OpmResult<SplittingConfig> {
        match self {
            Self::FixedRatio(c) => Ok(SplittingConfig::Ratio(*c)),
            Self::Spectrum(spectral_filter_builder) => {
                Ok(SplittingConfig::Spectrum(spectral_filter_builder.build()?))
            }
        }
    }
}

impl From<SpectralFilterBuilder> for SplittingConfigBuilder {
    fn from(val: SpectralFilterBuilder) -> Self {
        Self::Spectrum(val)
    }
}
impl From<f64> for SplittingConfigBuilder {
    fn from(val: f64) -> Self {
        Self::FixedRatio(val)
    }
}

impl Display for SplittingConfigBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixedRatio(_) => write!(f, "Fixed Ratio"),
            Self::Spectrum(_) => write!(f, "Spectral filter"),
        }
    }
}

impl DefaultFromName for SplittingConfigBuilder {
    fn default_from_name(name: &str) -> Option<Self> {
        for ftb in Self::iter() {
            if name == format!("{ftb}") {
                match ftb {
                    Self::FixedRatio(_) => {
                        return Some(Self::FixedRatio(0.5));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
/// Configuration for splitting a [`Ray`](crate::light::Ray) into multiple parts.
///
/// This enum defines how a ray is split, either by a fixed ratio or by a wavelength-dependent spectrum.
pub enum SplittingConfig {
    /// Ideal beam splitter with a fixed splitting ratio.
    ///
    /// The `f64` value must be in the range (0.0..=1.0), where 1.0 means all energy remains in the initial beam,
    /// and 0.0 means all energy is transferred to the split beam.
    Ratio(f64),
    /// Beam splitter with a wavelength-dependent transmission spectrum.
    ///
    /// The [`Spectrum`] must contain values in the range (0.0..=1.0).
    Spectrum(Spectrum),
}
impl SplittingConfig {
    /// Checks the validity of the [`SplittingConfig`].
    ///
    /// Returns `true` if all values in the spectrum or the ratio are within the range (0.0..=1.0).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Ratio(r) => (0.0..=1.0).contains(r),
            Self::Spectrum(s) => s.is_transmission_spectrum(),
        }
    }
    /// Returns the transmission of this [`SplittingConfig`] at a given wavelength.
    ///
    /// The transmission is the part of the energy that remains in the initial beam. For
    /// [`SplittingConfig::Ratio`] it is the ratio itself, for [`SplittingConfig::Spectrum`] it is the
    /// spectrum value at the given wavelength.
    ///
    /// # Arguments
    ///
    /// * `wavelength` - the wavelength of the light to be split.
    ///
    /// # Returns
    ///
    /// The transmission in the range `(0.0..=1.0)`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the wavelength is outside the given spectrum or the
    /// transmission is outside the interval `[0.0..1.0]`.
    pub fn transmission(&self, wavelength: Length) -> OpmResult<f64> {
        let transmission = match self {
            Self::Ratio(r) => *r,
            Self::Spectrum(spectrum) => spectrum.get_value(&wavelength).ok_or_else(|| {
                OpossumError::Spectrum(
                    "ray splitting failed. wavelength outside given spectrum".into(),
                )
            })?,
        };
        if !(0.0..=1.0).contains(&transmission) {
            return Err(OpossumError::Other(
                "splitting_ratio must be within [0.0;1.0]".into(),
            ));
        }
        Ok(transmission)
    }
    /// Returns the coating of a splitting surface with this [`SplittingConfig`] at a given wavelength.
    ///
    /// The splitting surface reflects everything that it does not transmit, so the returned
    /// coating has a constant reflectivity of `1 - transmission`.
    ///
    /// # Arguments
    ///
    /// * `wavelength` - the wavelength of the light hitting the splitting surface.
    ///
    /// # Returns
    ///
    /// A [`CoatingType::ConstantR`] with the reflectivity of the splitting surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the transmission cannot be determined (see
    /// [`transmission`](Self::transmission)).
    pub fn coating(&self, wavelength: Length) -> OpmResult<CoatingType> {
        let reflectivity = Ratio::new::<ratio>(1.0 - self.transmission(wavelength)?);
        Ok(CoatingType::ConstantR(CoatingConstantR::new(reflectivity)?))
    }
}

impl Display for SplittingConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ratio(_) => write!(f, "Fixed Ratio"),
            Self::Spectrum(_) => write!(f, "Split by Spectrum"),
        }
    }
}

impl From<SplittingConfigBuilder> for Proptype {
    fn from(config: SplittingConfigBuilder) -> Self {
        Self::SplittingConfigBuilder(config)
    }
}

inventory::submit! {
    NodeRegistration::new::<BeamSplitter>("beam splitter", "ideal beam splitter")
}

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightpink")]
/// An ideal beamsplitter node with a given splitting ratio.
///
/// The splitting surface is the plane of the node. Its transmission is given by the
/// [`SplittingConfig`]; the surface reflects the rest. The coatings of the ports are not used for
/// the split. In ray tracing, the transmitted light keeps its direction while the reflected light
/// is mirrored about the splitting surface. Hence, the beam splitter has to be tilted via its
/// alignment (e.g. by 45°) to send the reflected light sideways; an untilted beam splitter reflects
/// it back onto the incoming beam.
///
/// ## Optical Ports
///   - Inputs
///     - `input_1`
///     - `input_2`
///   - Outputs
///     - `out1_trans1_refl2`
///     - `out2_trans2_refl1`
///
/// ## Properties
///   - `name`
///   - `apertures`
///   - `inverted`
///   - `splitter config`
pub struct BeamSplitter {
    node_attr: NodeAttr,
}

impl Default for BeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("beam splitter");
        node_attr
            .create_property_with_validator(
                "splitter config builder",
                "config data of the beam splitter",
                Validator::NumericInRange { min: 0., max: 1. },
                SplittingConfigBuilder::FixedRatio(0.5).into(),
            )
            .unwrap();
        create_surface_properties(&mut node_attr).unwrap();
        let mut bs = Self { node_attr };
        bs.update_surfaces().unwrap();
        bs
    }
}
impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] with a given [`SplittingConfig`].
    ///
    /// ## Errors
    /// This function returns an [`OpossumError::Other`] if the [`SplittingConfig`] is invalid.
    pub fn new(name: &str, config: &SplittingConfigBuilder) -> OpmResult<Self> {
        let mut bs = Self::default();
        bs.node_attr.set_name(name);
        bs.node_attr
            .set_property("splitter config builder", config.clone().into())?;
        bs.update_surfaces()?;
        Ok(bs)
    }
    /// Returns the splitting config of this [`BeamSplitter`].
    ///
    /// See [`SplittingConfig`] for further details.
    /// # Errors
    /// This functions errors if the specified [`Properties`](crate::properties::Properties), do not exist or if the property has the wrong data format
    pub fn splitting_config(&self) -> OpmResult<SplittingConfig> {
        if let Ok(Proptype::SplittingConfigBuilder(config)) =
            self.node_attr.get_property("splitter config builder")
        {
            config.build()
        } else {
            Err(OpossumError::Other(
                "property `splitter config` does not exist or has wrong data format".into(),
            ))
        }
    }
    /// Sets the [`SplittingConfig`] of this [`BeamSplitter`].
    ///
    /// # Errors
    /// This function returns an [`OpossumError::Other`] if the [`SplittingConfig`] is invalid.
    pub fn set_splitting_config(&mut self, config: &SplittingConfigBuilder) -> OpmResult<()> {
        self.node_attr
            .set_property("splitter config builder", config.clone().into())?;
        Ok(())
    }
    fn split_spectrum(
        input: Option<&LightData>,
        splitting_config: &SplittingConfig,
    ) -> OpmResult<(Option<Spectrum>, Option<Spectrum>)> {
        if let Some(in1) = input {
            match in1 {
                LightData::Energy(spectrum) => {
                    match splitting_config {
                        SplittingConfig::Ratio(r) => {
                            let mut s = spectrum.clone();
                            s.scale_vertical(r)?;
                            let out1_spectrum = Some(s);
                            let mut s = spectrum.clone();
                            s.scale_vertical(&(1.0 - r))?;
                            let out2_spectrum = Some(s);
                            Ok((out1_spectrum, out2_spectrum))
                        },
                        SplittingConfig::Spectrum(spec) => {
                            let mut s = spectrum.clone();
                            let split_spectrum=s.split_by_spectrum(spec);
                            let out1_spectrum = Some(s);
                            let out2_spectrum = Some(split_spectrum);
                            Ok((out1_spectrum, out2_spectrum))
                        },
                    }
                },
                _ => {
                    Err(OpossumError::Analysis(
                        "expected LightData::Energy value at input port. A reason might be that the wrong analzer was used for the given light source data type. Try to use another analyzer (e.g. ray tracing)".into(),
                    ))
                }
            }
        } else {
            Ok((None, None))
        }
    }
    fn analyze_energy(
        &self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        let splitting_config = self.splitting_config()?;
        let (out1_1_spectrum, out1_2_spectrum) = Self::split_spectrum(in1, &splitting_config)?;
        let (out2_1_spectrum, out2_2_spectrum) = Self::split_spectrum(in2, &splitting_config)?;

        let out1_spec = merge_spectra(out1_1_spectrum, out2_2_spectrum);
        let out2_spec = merge_spectra(out1_2_spectrum, out2_1_spectrum);
        let mut out1_data: Option<LightData> = None;
        let mut out2_data: Option<LightData> = None;
        if let Some(out1_spec) = out1_spec {
            out1_data = Some(LightData::Energy(out1_spec));
        }
        if let Some(out2_spec) = out2_spec {
            out2_data = Some(LightData::Energy(out2_spec));
        }
        Ok((out1_data, out2_data))
    }
    /// Processes rays arriving at a single input port.
    ///
    /// The rays are split on the splitting surface of this port: the transmitted part keeps its
    /// direction, the reflected part is mirrored about the surface normal. Afterwards, the input
    /// aperture is applied to both parts.
    ///
    /// # Arguments
    ///
    /// * `input` - the light arriving at the port, if any.
    /// * `port_name` - the name of the input port (and its surface).
    /// * `splitting_config` - the [`SplittingConfig`] defining the transmission of the splitting surface.
    /// * `missed_surface_strategy` - what happens to rays that do not hit the splitting surface.
    ///
    /// # Returns
    ///
    /// The transmitted and the reflected rays, in that order. Both are empty if there is no input.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    ///   - the input is not [`LightData::Geometric`].
    ///   - the surface or the aperture of the port cannot be found.
    ///   - the splitting of the rays or the apodization fails.
    fn process_input_port(
        &mut self,
        input: Option<&LightData>,
        port_name: &str,
        splitting_config: &SplittingConfig,
        missed_surface_strategy: MissedSurfaceStrategy,
    ) -> OpmResult<(Rays, Rays)> {
        if let Some(light_data) = input {
            match light_data {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    // Split rays on the input surface
                    let mut reflected = if let Some(surf) = self.get_optic_surface_mut(port_name) {
                        rays.split_on_surface(surf, splitting_config, &missed_surface_strategy)?
                    } else {
                        return Err(OpossumError::OpticPort(format!(
                            "Input optic surface not found for port '{port_name}'"
                        )));
                    };

                    // Apply the input aperture to both parts. They sit at the same points on the
                    // surface, so this equals apodizing the rays before splitting them.
                    if let Some(aperture) = self.ports().aperture(&PortType::Input, port_name) {
                        let iso = self.effective_surface_iso(port_name)?;
                        rays.apodize(aperture, &iso)?;
                        reflected.apodize(aperture, &iso)?;
                    } else {
                        return Err(OpossumError::OpticPort(format!(
                            "Input aperture not found for port '{port_name}'"
                        )));
                    }
                    Ok((rays, reflected))
                }
                _ => Err(OpossumError::Analysis(format!(
                    "Expected LightData::Geometric at port '{port_name}'"
                ))),
            }
        } else {
            // No input, return empty sets of rays
            Ok((Rays::default(), Rays::default()))
        }
    }
    /// Processes rays for a single output port.
    /// This includes apodization and invalidating low-energy rays.
    fn process_output_port(
        &self,
        rays: &mut Rays,
        port_name: &str,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<()> {
        if let Some(aperture) = self.ports().aperture(&PortType::Output, port_name) {
            let iso = self.effective_surface_iso(port_name)?;
            rays.apodize(aperture, &iso)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort(format!(
                "Output aperture not found for port '{port_name}'"
            )));
        }
        Ok(())
    }
    fn analyze_raytrace(
        &mut self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        }

        let in1_port_name = &self.ports().names(&PortType::Input)[0];
        let in2_port_name = &self.ports().names(&PortType::Input)[1];
        let out1_port_name = &self.ports().names(&PortType::Output)[0];
        let out2_port_name = &self.ports().names(&PortType::Output)[1];

        let splitting_config = self.splitting_config()?;
        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy(_) => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => ray_trace_config.missed_surface_strategy(),
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };

        // Process both inputs
        let (mut main_rays1, split_rays1) = self.process_input_port(
            in1,
            in1_port_name,
            &splitting_config,
            *missed_surface_strategy,
        )?;
        let (mut main_rays2, split_rays2) = self.process_input_port(
            in2,
            in2_port_name,
            &splitting_config,
            *missed_surface_strategy,
        )?;

        // Merge the transmitted and reflected rays for the two outputs
        main_rays1.merge(&split_rays2); // out1 = trans1 + refl2
        main_rays2.merge(&split_rays1); // out2 = trans2 + refl1

        // Process both outputs
        self.process_output_port(&mut main_rays1, out1_port_name, analyzer_type)?;
        self.process_output_port(&mut main_rays2, out2_port_name, analyzer_type)?;

        Ok((
            Some(LightData::Geometric(main_rays1)),
            Some(LightData::Geometric(main_rays2)),
        ))
    }
}
impl Planar for BeamSplitter {
    fn surface_kind(&self) -> SurfaceKind {
        SurfaceKind::Reflective
    }
}
impl OpticNode for BeamSplitter {
    fn as_surface(&self) -> Option<&dyn Planar> {
        Some(self)
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);

        let input_surf_name_list = vec!["input_1", "input_2"];
        let output_surf_name_list = vec!["out1_trans1_refl2", "out2_trans2_refl1"];
        let geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));
        let anchor_point_iso = Isometry::identity();
        for in_surf_name in &input_surf_name_list {
            self.update_surface(
                in_surf_name,
                geosurface.clone(),
                anchor_point_iso,
                &PortType::Input,
            )?;
        }
        for out_surf_name in &output_surf_name_list {
            self.update_surface(
                out_surf_name,
                geosurface.clone(),
                anchor_point_iso,
                &PortType::Output,
            )?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        core_optics::{NodeAttrExt, PortType},
        nanometer,
        nodes::{
            ideal_filter::{EdgeFilter, EdgeFilterType},
            test_helper::test_helper::*,
        },
    };
    use approx::assert_abs_diff_eq;
    #[test]
    fn default() -> OpmResult<()> {
        let node = BeamSplitter::default();
        assert!(matches!(
            node.splitting_config()?,
            SplittingConfig::Ratio(_)
        ));
        assert_eq!(node.name(), "beam splitter");
        assert_eq!(node.node_type(), "beam splitter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightpink");
        Ok(())
    }
    #[test]
    fn new() -> OpmResult<()> {
        let splitter = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6))?;
        assert_eq!(splitter.name(), "test");
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(-0.01)).is_err());
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(1.01)).is_err());
        Ok(())
    }
    #[test]
    fn inverted() -> OpmResult<()> {
        test_inverted::<BeamSplitter>()
    }
    #[test]
    fn ports() {
        let node = BeamSplitter::default();
        let mut input_ports = node.ports().names(&PortType::Input);
        input_ports.sort();
        assert_eq!(input_ports, vec!["input_1", "input_2"]);
        let mut output_ports = node.ports().names(&PortType::Output);
        output_ports.sort();
        assert_eq!(output_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
    }
    #[test]
    fn ports_inverted() -> OpmResult<()> {
        let mut node = BeamSplitter::default();
        node.set_inverted(true)?;
        let mut input_ports = node.ports().names(&PortType::Input);
        input_ports.sort();
        assert_eq!(input_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
        let mut output_ports = node.ports().names(&PortType::Output);
        output_ports.sort();
        assert_eq!(output_ports, vec!["input_1", "input_2"]);
        Ok(())
    }
    #[test]
    fn analyze_empty() -> OpmResult<()> {
        test_analyze_empty::<BeamSplitter>()
    }
    fn short_pass_config() -> OpmResult<SplittingConfig> {
        let spectrum: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            nanometer!(1000.0),
            0.0..1.0,
            None,
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
        )?
        .into();
        Ok(SplittingConfig::Spectrum(spectrum))
    }
    #[test]
    fn splitting_config_transmission() -> OpmResult<()> {
        assert_abs_diff_eq!(
            SplittingConfig::Ratio(0.6).transmission(nanometer!(1000.0))?,
            0.6
        );
        assert!(
            SplittingConfig::Ratio(1.1)
                .transmission(nanometer!(1000.0))
                .is_err()
        );
        assert!(
            SplittingConfig::Ratio(-0.1)
                .transmission(nanometer!(1000.0))
                .is_err()
        );
        let config = short_pass_config()?;
        assert_abs_diff_eq!(config.transmission(nanometer!(999.0))?, 1.0);
        assert_abs_diff_eq!(config.transmission(nanometer!(1001.0))?, 0.0);
        assert!(config.transmission(nanometer!(1501.0)).is_err());
        Ok(())
    }
    #[test]
    fn splitting_config_coating() -> OpmResult<()> {
        let CoatingType::ConstantR(coating) =
            SplittingConfig::Ratio(0.6).coating(nanometer!(1000.0))?
        else {
            panic!("expected a constant reflectivity coating");
        };
        assert_abs_diff_eq!(coating.reflectivity().get::<ratio>(), 0.4);
        assert!(short_pass_config()?.coating(nanometer!(1501.0)).is_err());
        Ok(())
    }
}
