#![warn(missing_docs)]

mod analysis_energy;
mod analysis_ghostfocus;
mod analysis_raytrace;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{AnalyzerType, raytrace::MissedSurfaceStrategy},
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    nodes::ideal_filter::SpectralFilterBuilder,
    optic_node::OpticNode,
    optic_ports::PortType,
    properties::{Proptype, validator::Validator},
    rays::Rays,
    spectrum::{Spectrum, merge_spectra},
    surface::{Plane, geo_surface::GeoSurfaceRef},
    utils::{default_from_name::DefaultFromName, geom_transformation::Isometry},
};
use opm_macros_lib::OpmNode;
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    sync::{Arc, Mutex},
};
use strum::{EnumIter, IntoEnumIterator};

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
                        return Some(Self::FixedRatio(1.0));
                    }
                    Self::Spectrum(_) => return Some(ftb),
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumIter)]
/// Configuration for splitting a [`Ray`] into multiple parts.
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

#[derive(OpmNode, Debug, Clone)]
#[opm_node("lightpink")]
/// An ideal beamsplitter node with a given splitting ratio.
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
unsafe impl Send for BeamSplitter {}

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
    #[allow(clippy::too_many_lines)]
    fn analyze_raytrace(
        &mut self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        let in1_port = &self.ports().names(&PortType::Input)[0];
        let in2_port = &self.ports().names(&PortType::Input)[1];
        let out1_port = &self.ports().names(&PortType::Output)[0];
        let out2_port = &self.ports().names(&PortType::Output)[1];

        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        }
        let splitting_config = self.splitting_config()?;
        let refraction_intended = true;
        let missed_surface_strategy = match analyzer_type {
            AnalyzerType::Energy => &MissedSurfaceStrategy::Stop,
            AnalyzerType::RayTrace(ray_trace_config) => ray_trace_config.missed_surface_strategy(),
            AnalyzerType::GhostFocus(_) => &MissedSurfaceStrategy::Ignore,
        };
        let (mut in_ray1, split1) = if let Some(input_1) = in1 {
            match input_1 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    if let Some(surf) = self.get_optic_surface_mut(in1_port) {
                        rays.refract_on_surface(
                            surf,
                            None,
                            refraction_intended,
                            missed_surface_strategy,
                        )?;

                        match self.ports().aperture(&PortType::Input, in1_port) {
                            Some(aperture) => {
                                rays.apodize(aperture, &self.effective_surface_iso(in1_port)?)?;
                            }
                            _ => {
                                return Err(OpossumError::OpticPort(
                                    "input aperture not found".into(),
                                ));
                            }
                        }
                    } else {
                        return Err(OpossumError::OpticPort(
                            "input optic surface not found".into(),
                        ));
                    }

                    let split_rays = rays.split(&splitting_config)?;
                    (rays, split_rays)
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input_1` port".into(),
                    ));
                }
            }
        } else {
            (Rays::default(), Rays::default())
        };
        let (mut in_ray2, split2) = if let Some(input_2) = in2 {
            match input_2 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    if let Some(surf) = self.get_optic_surface_mut(in2_port) {
                        rays.refract_on_surface(
                            surf,
                            None,
                            refraction_intended,
                            missed_surface_strategy,
                        )?;
                        match self.ports().aperture(&PortType::Input, in2_port) {
                            Some(aperture) => {
                                rays.apodize(aperture, &self.effective_surface_iso(in2_port)?)?;
                            }
                            _ => {
                                return Err(OpossumError::OpticPort(
                                    "input aperture not found".into(),
                                ));
                            }
                        }
                        let split_rays = rays.split(&splitting_config)?;
                        (rays, split_rays)
                    } else {
                        return Err(OpossumError::OpticPort(
                            "input optic surface not found".into(),
                        ));
                    }
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input_2` port".into(),
                    ));
                }
            }
        } else {
            (Rays::default(), Rays::default())
        };
        in_ray1.merge(&split2);
        in_ray2.merge(&split1);
        let iso = self.effective_surface_iso(out1_port)?;

        match self.ports().aperture(&PortType::Output, out1_port) {
            Some(aperture) => {
                in_ray1.apodize(aperture, &iso)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    in_ray1.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            }
            _ => {
                return Err(OpossumError::OpticPort("ouput aperture not found".into()));
            }
        }
        match self.ports().aperture(&PortType::Output, out2_port) {
            Some(aperture) => {
                in_ray2.apodize(aperture, &iso)?;
                if let AnalyzerType::RayTrace(config) = analyzer_type {
                    in_ray2.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                }
            }
            _ => {
                return Err(OpossumError::OpticPort("ouput aperture not found".into()));
            }
        }
        Ok((
            Some(LightData::Geometric(in_ray1)),
            Some(LightData::Geometric(in_ray2)),
        ))
    }
}
impl OpticNode for BeamSplitter {
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn node_attr_mut(&mut self) -> &mut NodeAttr {
        &mut self.node_attr
    }
    fn update_surfaces(&mut self) -> OpmResult<()> {
        let node_iso = self.effective_node_iso().unwrap_or_else(Isometry::identity);

        let input_surf_name_list = vec!["input_1", "input_2"];
        let output_surf_name_list = vec!["out1_trans1_refl2", "out2_trans2_refl1"];
        let geosurface = GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(node_iso))));
        let anchor_point_iso = Isometry::identity();
        for in_surf_name in &input_surf_name_list {
            self.update_surface(
                &(*in_surf_name).to_string(),
                geosurface.clone(),
                anchor_point_iso,
                &PortType::Input,
            )?;
        }
        for out_surf_name in &output_surf_name_list {
            self.update_surface(
                &(*out_surf_name).to_string(),
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
    use crate::{nodes::test_helper::test_helper::*, optic_ports::PortType};
    #[test]
    fn default() {
        let mut node = BeamSplitter::default();
        assert!(matches!(
            node.splitting_config().unwrap(),
            SplittingConfig::Ratio(_)
        ));
        assert_eq!(node.name(), "beam splitter");
        assert_eq!(node.node_type(), "beam splitter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightpink");
        assert!(node.as_group_mut().is_err());
    }
    #[test]
    fn new() {
        let splitter = BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(0.6));
        assert!(splitter.is_ok());
        let splitter = splitter.unwrap();
        assert_eq!(splitter.name(), "test");
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(-0.01)).is_err());
        assert!(BeamSplitter::new("test", &SplittingConfigBuilder::FixedRatio(1.01)).is_err());
    }
    #[test]
    fn inverted() {
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
    fn ports_inverted() {
        let mut node = BeamSplitter::default();
        node.set_inverted(true).unwrap();
        let mut input_ports = node.ports().names(&PortType::Input);
        input_ports.sort();
        assert_eq!(input_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
        let mut output_ports = node.ports().names(&PortType::Output);
        output_ports.sort();
        assert_eq!(output_ports, vec!["input_1", "input_2"]);
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<BeamSplitter>()
    }
}
