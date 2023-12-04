use std::collections::HashMap;

use uom::si::{f64::Length, length::millimeter};

use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
};

#[derive(Debug, Clone)]
pub struct ParaxialSurface {
    props: Properties,
}
impl Default for ParaxialSurface {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new("paraxial surface", "paraxial");
        props
            .create("focal length", "focal length in mm", None, 1.0.into())
            .unwrap();
        props.set("apertures", ports.into()).unwrap();
        Self { props }
    }
}
impl ParaxialSurface {
    /// Create a new paraxial surface node of the given focal length.
    ///
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given `focal_length` is not finite.
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the property `apertures` can not be set.
    pub fn new(name: &str, focal_length: Length) -> OpmResult<Self> {
        if !focal_length.is_finite() {
            return Err(OpossumError::Other("focal length must be finite".into()));
        }
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new(name, "paraxial");
        props
            .create(
                "focal length",
                "focal length in mm",
                None,
                focal_length.get::<millimeter>().into(),
            )
            .unwrap();
        props.set("apertures", ports.into()).unwrap();
        Ok(Self { props })
    }
}

impl Optical for ParaxialSurface {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let (src, target) = if self.properties().inverted()? {
            ("rear", "front")
        } else {
            ("front", "rear")
        };
        let mut data = incoming_data.get(src).unwrap_or(&None).clone();
        match analyzer_type {
            AnalyzerType::Energy => (),
            AnalyzerType::RayTrace(_config) => {
                if let Some(LightData::Geometric(mut rays)) = data {
                    let focal_length =
                        if let Ok(Proptype::F64(length)) = self.props.get("focal length") {
                            *length
                        } else {
                            return Err(OpossumError::Analysis("cannot read focal length".into()));
                        };
                    rays.refract_paraxial(Length::new::<millimeter>(focal_length))?;
                    data = Some(LightData::Geometric(rays));
                } else {
                    return Err(crate::error::OpossumError::Analysis(
                        "No LightData::Geometric for analyzer type RayTrace".into(),
                    ));
                }
            }
        }
        Ok(HashMap::from([(target.into(), data)]))
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for ParaxialSurface {
    fn node_color(&self) -> &str {
        "palegreen"
    }
}
