use std::collections::HashMap;

use uom::si::{
    f64::Length,
    length::{meter, millimeter},
};

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
pub struct Propagation {
    props: Properties,
}
impl Default for Propagation {
    fn default() -> Self {
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new("propagation", "propagation");
        props
            .create(
                "distance",
                "distance along the optical axis",
                None,
                1.0.into(),
            )
            .unwrap();
        props.set("apertures", ports.into()).unwrap();
        Self { props }
    }
}
impl Propagation {
    /// Create a new propagation node of the given length.
    ///
    ///
    /// # Errors
    /// This function returns an error if
    ///  - the given `length_along_z` is not finite.
    /// # Panics
    /// This function panics if
    /// - the input port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the output port name already exists. (Theoretically impossible at this point, as the [`OpticPorts`] are created just before in this function)
    /// - the property `apertures` can not be set.
    pub fn new(name: &str, length_along_z: Length) -> OpmResult<Self> {
        if !length_along_z.is_finite() {
            return Err(OpossumError::Other(
                "propagation length must be finite".into(),
            ));
        }
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        let mut props = Properties::new(name, "propagation");
        props
            .create(
                "distance",
                "distance along the optical axis",
                None,
                length_along_z.get::<meter>().into(),
            )
            .unwrap();
        props.set("apertures", ports.into()).unwrap();
        Ok(Self { props })
    }
}

impl Optical for Propagation {
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
                    let length_along_z =
                        if let Ok(Proptype::F64(length)) = self.props.get("distance") {
                            *length
                        } else {
                            return Err(OpossumError::Analysis("cannot read distance".into()));
                        };
                    rays.propagate_along_z(Length::new::<millimeter>(length_along_z))?;
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

impl Dottable for Propagation {}
