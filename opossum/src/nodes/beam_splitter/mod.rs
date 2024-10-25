#![warn(missing_docs)]

mod analysis_energy;
mod analysis_raytrace;

use super::node_attr::NodeAttr;
use crate::{
    analyzers::{ghostfocus::AnalysisGhostFocus, Analyzable, AnalyzerType},
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::{DataEnergy, LightData},
    optic_node::OpticNode,
    optic_ports::{OpticPorts, PortType},
    properties::Proptype,
    ray::SplittingConfig,
    rays::Rays,
    spectrum::{merge_spectra, Spectrum},
    surface::{OpticalSurface, Plane, Surface},
    utils::EnumProxy,
};

#[derive(Debug, Clone)]
/// An ideal beamsplitter node with a given splitting ratio.
///
/// ## Optical Ports
///   - Inputs
///     - `input1`
///     - `input2`
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
            .create_property(
                "splitter config",
                "config data of the beam splitter",
                None,
                EnumProxy::<SplittingConfig> {
                    value: SplittingConfig::Ratio(0.5),
                }
                .into(),
            )
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.add(&PortType::Input, "input1").unwrap();
        ports.add(&PortType::Input, "input2").unwrap();
        ports.add(&PortType::Output, "out1_trans1_refl2").unwrap();
        ports.add(&PortType::Output, "out2_trans2_refl1").unwrap();
        node_attr.set_ports(ports);
        Self { node_attr }
    }
}
impl BeamSplitter {
    /// Creates a new [`BeamSplitter`] with a given [`SplittingConfig`].
    ///
    /// ## Errors
    /// This function returns an [`OpossumError::Other`] if the [`SplittingConfig`] is invalid.
    pub fn new(name: &str, config: &SplittingConfig) -> OpmResult<Self> {
        if !config.is_valid() {
            return Err(OpossumError::Properties(
                "ratio must be within (0.0..=1.0)".into(),
            ));
        }
        let mut bs = Self::default();
        bs.node_attr.set_property(
            "splitter config",
            EnumProxy::<SplittingConfig> {
                value: config.clone(),
            }
            .into(),
        )?;
        bs.node_attr.set_name(name);
        Ok(bs)
    }
    /// Returns the splitting config of this [`BeamSplitter`].
    ///
    /// See [`SplittingConfig`] for further details.
    /// # Panics
    /// This functions panics if the specified [`Properties`](crate::properties::Properties), here `ratio`, do not exist or if the property has the wrong data format
    #[must_use]
    pub fn splitting_config(&self) -> SplittingConfig {
        if let Ok(Proptype::SplitterType(config)) = self.node_attr.get_property("splitter config") {
            return config.value.clone();
        }
        panic!("property `splitter config` does not exist or has wrong data format")
    }
    /// Sets the [`SplittingConfig`] of this [`BeamSplitter`].
    ///
    /// # Errors
    /// This function returns an [`OpossumError::Other`] if the [`SplittingConfig`] is invalid.
    pub fn set_splitting_config(&mut self, config: &SplittingConfig) -> OpmResult<()> {
        // if ratio.is_sign_negative() || ratio > 1.0 || !ratio.is_finite() {
        //     return Err(OpossumError::Properties(
        //         "ratio must be within (0.0..1.0) and finite".into(),
        //     ));
        // }
        self.node_attr.set_property(
            "splitter config",
            EnumProxy::<SplittingConfig> {
                value: config.clone(),
            }
            .into(),
        )?;
        Ok(())
    }
    fn split_spectrum(
        &self,
        input: Option<&LightData>,
    ) -> OpmResult<(Option<Spectrum>, Option<Spectrum>)> {
        if let Some(in1) = input {
            match in1 {
                LightData::Energy(e) => {
                    match self.splitting_config() {
                        SplittingConfig::Ratio(r) => {
                            let mut s = e.spectrum.clone();
                            s.scale_vertical(&r)?;
                            let out1_spectrum = Some(s);
                            let mut s = e.spectrum.clone();
                            s.scale_vertical(&(1.0 - r))?;
                            let out2_spectrum = Some(s);
                            Ok((out1_spectrum, out2_spectrum))
                        },
                        SplittingConfig::Spectrum(spec) => {
                            let mut s = e.spectrum.clone();
                            let split_spectrum=s.split_by_spectrum(&spec);
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
        let (out1_1_spectrum, out1_2_spectrum) = self.split_spectrum(in1)?;
        let (out2_1_spectrum, out2_2_spectrum) = self.split_spectrum(in2)?;

        let out1_spec = merge_spectra(out1_1_spectrum, out2_2_spectrum);
        let out2_spec = merge_spectra(out1_2_spectrum, out2_1_spectrum);
        let mut out1_data: Option<LightData> = None;
        let mut out2_data: Option<LightData> = None;
        if let Some(out1_spec) = out1_spec {
            out1_data = Some(LightData::Energy(DataEnergy {
                spectrum: out1_spec,
            }));
        }
        if let Some(out2_spec) = out2_spec {
            out2_data = Some(LightData::Energy(DataEnergy {
                spectrum: out2_spec,
            }));
        }
        Ok((out1_data, out2_data))
    }
    #[allow(clippy::too_many_lines)]
    fn analyze_raytrace(
        &self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        };
        let Ok(Proptype::SplitterType(splitting_config)) =
            self.node_attr.get_property("splitter config")
        else {
            return Err(OpossumError::Analysis(
                "could not read splitter config property".into(),
            ));
        };
        let (mut in_ray1, split1) = if let Some(input1) = in1 {
            match input1 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    if let Some(iso) = self.effective_iso() {
                        let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
                        rays.refract_on_surface(&mut plane, None)?;
                        if let Some(aperture) = self.ports().aperture(&PortType::Input, "input1") {
                            rays.apodize(aperture, &iso)?;
                            if let AnalyzerType::RayTrace(config) = analyzer_type {
                                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                            }
                        } else {
                            return Err(OpossumError::OpticPort("input aperture not found".into()));
                        };
                    } else {
                        return Err(OpossumError::Analysis(
                            "no location for surface defined. Aborting".into(),
                        ));
                    }

                    let split_rays = rays.split(&splitting_config.value)?;
                    (rays, split_rays)
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input1` port".into(),
                    ))
                }
            }
        } else {
            (Rays::default(), Rays::default())
        };
        let (mut in_ray2, split2) = if let Some(input2) = in2 {
            match input2 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    let Some(iso) = self.effective_iso() else {
                        return Err(OpossumError::Analysis(
                            "no location for surface defined. Aborting".into(),
                        ));
                    };

                    let mut plane = OpticalSurface::new(Box::new(Plane::new(&iso)));
                    rays.refract_on_surface(&mut plane, None)?;
                    if let Some(aperture) = self.ports().aperture(&PortType::Input, "input2") {
                        rays.apodize(aperture, &iso)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("input aperture not found".into()));
                    };

                    let split_rays = rays.split(&splitting_config.value)?;
                    (rays, split_rays)
                }
                _ => {
                    return Err(OpossumError::Analysis(
                        "expected Rays value at `input2` port".into(),
                    ))
                }
            }
        } else {
            (Rays::default(), Rays::default())
        };
        in_ray1.merge(&split2);
        in_ray2.merge(&split1);
        let Some(iso) = self.effective_iso() else {
            return Err(OpossumError::Analysis(
                "no location for surface defined. Aborting".into(),
            ));
        };
        if let Some(aperture) = self
            .ports()
            .aperture(&PortType::Output, "out1_trans1_refl2")
        {
            in_ray1.apodize(aperture, &iso)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                in_ray1.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("ouput aperture not found".into()));
        };
        if let Some(aperture) = self
            .ports()
            .aperture(&PortType::Output, "out2_trans2_refl1")
        {
            in_ray2.apodize(aperture, &iso)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                in_ray2.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("ouput aperture not found".into()));
        };
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
}

impl Dottable for BeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}
impl Surface for BeamSplitter {
    fn get_surface_mut(&mut self, _surf_name: &str) -> &mut OpticalSurface {
        todo!()
    }
}
impl Analyzable for BeamSplitter {}
impl AnalysisGhostFocus for BeamSplitter {}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::energy::AnalysisEnergy, light_result::LightResult,
        nodes::test_helper::test_helper::*, optic_ports::PortType,
        spectrum_helper::create_he_ne_spec,
    };
    use approx::assert_abs_diff_eq;
    #[test]
    fn default() {
        let mut node = BeamSplitter::default();
        assert!(matches!(node.splitting_config(), SplittingConfig::Ratio(_)));
        assert_eq!(node.name(), "beam splitter");
        assert_eq!(node.node_type(), "beam splitter");
        assert_eq!(node.inverted(), false);
        assert_eq!(node.node_color(), "lightpink");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let splitter = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6));
        assert!(splitter.is_ok());
        let splitter = splitter.unwrap();
        assert_eq!(splitter.name(), "test");
        assert!(BeamSplitter::new("test", &SplittingConfig::Ratio(-0.01)).is_err());
        assert!(BeamSplitter::new("test", &SplittingConfig::Ratio(1.01)).is_err());
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
        assert_eq!(input_ports, vec!["input1", "input2"]);
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
        assert_eq!(output_ports, vec!["input1", "input2"]);
    }
    #[test]
    fn analyze_empty() {
        test_analyze_empty::<BeamSplitter>()
    }

    #[test]
    fn analyze_inverse() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        node.set_inverted(true).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "out1_trans1_refl2".into(),
            LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            }),
        );
        input.insert(
            "out2_trans2_refl1".into(),
            LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(0.5).unwrap(),
            }),
        );
        let output = AnalysisEnergy::analyze(&mut node, input).unwrap();
        let energy_output1 =
            if let LightData::Energy(s) = output.clone().get("input1").unwrap().clone() {
                s.spectrum.total_energy()
            } else {
                0.0
            };

        let energy_output2 =
            if let LightData::Energy(s) = output.clone().get("input2").unwrap().clone() {
                s.spectrum.total_energy()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output1, &0.8);
        assert_abs_diff_eq!(energy_output2, &0.7);
    }
}
