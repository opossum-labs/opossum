#![warn(missing_docs)]
use crate::ray::SplittingConfig;
use crate::refractive_index::refr_index_vaccuum;
use crate::surface::Plane;
use crate::utils::EnumProxy;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::{DataEnergy, LightData},
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    rays::Rays,
    spectrum::{merge_spectra, Spectrum},
};
use std::collections::HashMap;

#[derive(Debug)]
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
    props: Properties,
}

fn create_default_props() -> Properties {
    let mut props = Properties::new("beam splitter", "beam splitter");
    props
        .create(
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
    ports.create_input("input1").unwrap();
    ports.create_input("input2").unwrap();
    ports.create_output("out1_trans1_refl2").unwrap();
    ports.create_output("out2_trans2_refl1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
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
        let mut props = create_default_props();
        props.set(
            "splitter config",
            EnumProxy::<SplittingConfig> {
                value: config.clone(),
            }
            .into(),
        )?;
        props.set("name", name.into())?;
        Ok(Self { props })
    }
    /// Returns the splitting config of this [`BeamSplitter`].
    ///
    /// See [`SplittingConfig`] for further details.
    /// # Panics
    /// This functions panics if the specified [`Properties`], here `ratio`, do not exist or if the property has the wrong data format
    #[must_use]
    pub fn splitting_config(&self) -> SplittingConfig {
        if let Ok(Proptype::SplitterType(config)) = self.props.get("splitter config") {
            return config.value.clone();
        }
        panic!("property `splitter config` does not exist or has wrong data format")
    }
    /// Sets the [`SplittingConfig`] of this [`BeamSplitter`].
    ///
    /// ## Errors
    /// This function returns an [`OpossumError::Other`] if the [`SplittingConfig`] is invalid.
    pub fn set_splitting_config(&mut self, config: &SplittingConfig) -> OpmResult<()> {
        // if ratio.is_sign_negative() || ratio > 1.0 || !ratio.is_finite() {
        //     return Err(OpossumError::Properties(
        //         "ratio must be within (0.0..1.0) and finite".into(),
        //     ));
        // }
        self.props.set(
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
        &mut self,
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
    fn analyze_raytrace(
        &mut self,
        in1: Option<&LightData>,
        in2: Option<&LightData>,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<(Option<LightData>, Option<LightData>)> {
        if in1.is_none() && in2.is_none() {
            return Ok((None, None));
        };
        let Ok(Proptype::SplitterType(splitting_config)) = self.props.get("splitter config") else {
            return Err(OpossumError::Analysis(
                "could not read splitter config property".into(),
            ));
        };
        let (mut in_ray1, split1) = if let Some(input1) = in1 {
            match input1 {
                LightData::Geometric(r) => {
                    let mut rays = r.clone();
                    let z_position =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    let plane = Plane::new(z_position)?;
                    rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
                    if let Some(aperture) = self.ports().input_aperture("input1") {
                        rays.apodize(aperture)?;
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
                    let z_position =
                        rays.absolute_z_of_last_surface() + rays.dist_to_next_surface();
                    let plane = Plane::new(z_position)?;
                    rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
                    if let Some(aperture) = self.ports().input_aperture("input2") {
                        rays.apodize(aperture)?;
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
        if let Some(aperture) = self.ports().output_aperture("out1_trans1_refl2") {
            in_ray1.apodize(aperture)?;
            if let AnalyzerType::RayTrace(config) = analyzer_type {
                in_ray1.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
            }
        } else {
            return Err(OpossumError::OpticPort("ouput aperture not found".into()));
        };
        if let Some(aperture) = self.ports().output_aperture("out2_trans2_refl1") {
            in_ray2.apodize(aperture)?;
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

impl Default for BeamSplitter {
    /// Create a 50:50 beamsplitter.
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Optical for BeamSplitter {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (input_port1, input_port2) = if self.properties().inverted()? {
            ("out1_trans1_refl2", "out2_trans2_refl1")
        } else {
            ("input1", "input2")
        };
        let in1 = if let Some(input) = incoming_data.get(input_port1) {
            if let Some(light_data) = input {
                Some(light_data)
            } else {
                return Err(OpossumError::Analysis(format!(
                    "beam splitter: no light data at input port <{input_port1}>"
                )));
            }
        } else {
            None
        };
        let in2 = if let Some(input) = incoming_data.get(input_port2) {
            if let Some(light_data) = input {
                Some(light_data)
            } else {
                return Err(OpossumError::Analysis(format!(
                    "beam splitter: no light data at input port <{input_port2}>"
                )));
            }
        } else {
            None
        };
        let (out1_data, out2_data) = match analyzer_type {
            AnalyzerType::Energy => self.analyze_energy(in1, in2)?,
            AnalyzerType::RayTrace(_) => self.analyze_raytrace(in1, in2, analyzer_type)?,
        };
        if self.properties().inverted()? {
            Ok(HashMap::from([
                ("input1".into(), out1_data),
                ("input2".into(), out2_data),
            ]))
        } else {
            Ok(HashMap::from([
                ("out1_trans1_refl2".into(), out1_data),
                ("out2_trans2_refl1".into(), out2_data),
            ]))
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for BeamSplitter {
    fn node_color(&self) -> &str {
        "lightpink"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::RayTraceConfig, joule, millimeter, nanometer, ray::Ray,
        spectrum_helper::create_he_ne_spec,
    };
    use approx::{assert_abs_diff_eq, AbsDiffEq};
    use uom::si::energy::joule;
    #[test]
    fn default() {
        let node = BeamSplitter::default();
        assert!(matches!(node.splitting_config(), SplittingConfig::Ratio(_)));
        assert_eq!(node.properties().name().unwrap(), "beam splitter");
        assert_eq!(node.properties().node_type().unwrap(), "beam splitter");
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "lightpink");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let splitter = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6));
        assert!(splitter.is_ok());
        let splitter = splitter.unwrap();
        assert_eq!(splitter.properties().name().unwrap(), "test");
        //assert_eq!(splitter.ratio(), 0.6);
        assert!(BeamSplitter::new("test", &SplittingConfig::Ratio(-0.01)).is_err());
        assert!(BeamSplitter::new("test", &SplittingConfig::Ratio(1.01)).is_err());
    }
    #[test]
    fn inverted() {
        let mut node = BeamSplitter::default();
        node.set_property("inverted", true.into()).unwrap();
        assert_eq!(node.properties().inverted().unwrap(), true)
    }
    #[test]
    fn ports() {
        let node = BeamSplitter::default();
        let mut input_ports = node.ports().input_names();
        input_ports.sort();
        assert_eq!(input_ports, vec!["input1", "input2"]);
        let mut output_ports = node.ports().output_names();
        output_ports.sort();
        assert_eq!(output_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
    }
    #[test]
    fn ports_inverted() {
        let mut node = BeamSplitter::default();
        node.set_property("inverted", true.into()).unwrap();
        let mut input_ports = node.ports().input_names();
        input_ports.sort();
        assert_eq!(input_ports, vec!["out1_trans1_refl2", "out2_trans2_refl1"]);
        let mut output_ports = node.ports().output_names();
        output_ports.sort();
        assert_eq!(output_ports, vec!["input1", "input2"]);
    }
    #[test]
    fn analyze_energy_empty_input() {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        assert!(output.contains_key("out1_trans1_refl2"));
        assert!(output.contains_key("out2_trans2_refl1"));
        assert!(output.clone().get("out1_trans1_refl2").unwrap().is_none());
        assert!(output.get("out2_trans2_refl1").unwrap().is_none());
    }
    #[test]
    fn analyze_energy_one_input() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "input1".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            })),
        );
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        let result = output
            .clone()
            .get("out1_trans1_refl2")
            .unwrap()
            .clone()
            .unwrap();
        let energy = if let LightData::Energy(e) = result {
            e.spectrum.total_energy()
        } else {
            0.0
        };
        assert_eq!(energy, 0.6);
        let result = output
            .clone()
            .get("out2_trans2_refl1")
            .unwrap()
            .clone()
            .unwrap();
        let energy = if let LightData::Energy(e) = result {
            e.spectrum.total_energy()
        } else {
            0.0
        };
        assert_eq!(energy, 0.4);
    }
    #[test]
    fn analyze_energy_two_input() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "input1".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            })),
        );
        input.insert(
            "input2".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(0.5).unwrap(),
            })),
        );
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        let energy_output1 = if let LightData::Energy(s) = output
            .clone()
            .get("out1_trans1_refl2")
            .unwrap()
            .clone()
            .unwrap()
        {
            s.spectrum.total_energy()
        } else {
            0.0
        };
        assert!(energy_output1.abs_diff_eq(&0.8, f64::EPSILON));
        let energy_output2 = if let LightData::Energy(s) = output
            .clone()
            .get("out2_trans2_refl1")
            .unwrap()
            .clone()
            .unwrap()
        {
            s.spectrum.total_energy()
        } else {
            0.0
        };
        assert!(energy_output2.abs_diff_eq(&0.7, f64::EPSILON));
    }
    #[test]
    fn analyze_raytrace_empty() {
        let mut node = BeamSplitter::default();
        let input = LightResult::default();
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        assert!(output.contains_key("out1_trans1_refl2"));
        assert!(output.contains_key("out2_trans2_refl1"));
        assert!(output.clone().get("out1_trans1_refl2").unwrap().is_none());
        assert!(output.get("out2_trans2_refl1").unwrap().is_none());
    }
    #[test]
    fn analyze_raytrace_one_input() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        input.insert("input1".into(), Some(LightData::Geometric(rays)));
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        let result = output.clone().get("out1_trans1_refl2").unwrap().clone();
        let energy = if let Some(LightData::Geometric(r)) = result {
            r.total_energy().get::<joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.6);
        let result = output.clone().get("out2_trans2_refl1").unwrap().clone();
        let energy = if let Some(LightData::Geometric(r)) = result {
            r.total_energy().get::<joule>()
        } else {
            0.0
        };
        assert_eq!(energy, 0.4);
    }
    #[test]
    fn analyze_raytrace_two_input() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        let mut input = LightResult::default();
        let mut rays = Rays::default();
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(1.0)).unwrap();
        rays.add_ray(ray);
        input.insert("input1".into(), Some(LightData::Geometric(rays)));
        let mut rays = Rays::default();
        let ray =
            Ray::new_collimated(millimeter!(0., 0., 0.), nanometer!(1053.0), joule!(0.5)).unwrap();
        rays.add_ray(ray);
        input.insert("input2".into(), Some(LightData::Geometric(rays)));
        let output = node
            .analyze(input, &AnalyzerType::RayTrace(RayTraceConfig::default()))
            .unwrap();
        let energy_output1 = if let Some(LightData::Geometric(r)) =
            output.clone().get("out1_trans1_refl2").unwrap().clone()
        {
            r.total_energy().get::<joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output1, &0.8);
        let energy_output2 = if let Some(LightData::Geometric(r)) =
            output.clone().get("out2_trans2_refl1").unwrap().clone()
        {
            r.total_energy().get::<joule>()
        } else {
            0.0
        };
        assert_abs_diff_eq!(energy_output2, &0.7);
    }
    #[test]
    fn analyze_inverse() {
        let mut node = BeamSplitter::new("test", &SplittingConfig::Ratio(0.6)).unwrap();
        node.set_property("inverted", true.into()).unwrap();
        let mut input = LightResult::default();
        input.insert(
            "out1_trans1_refl2".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(1.0).unwrap(),
            })),
        );
        input.insert(
            "out2_trans2_refl1".into(),
            Some(LightData::Energy(DataEnergy {
                spectrum: create_he_ne_spec(0.5).unwrap(),
            })),
        );
        let output = node.analyze(input, &AnalyzerType::Energy).unwrap();
        let energy_output1 =
            if let Some(LightData::Energy(s)) = output.clone().get("input1").unwrap().clone() {
                s.spectrum.total_energy()
            } else {
                0.0
            };

        let energy_output2 =
            if let Some(LightData::Energy(s)) = output.clone().get("input2").unwrap().clone() {
                s.spectrum.total_energy()
            } else {
                0.0
            };
        assert_abs_diff_eq!(energy_output1, &0.8);
        assert_abs_diff_eq!(energy_output2, &0.7);
    }
}
