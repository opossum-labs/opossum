#![warn(missing_docs)]
use crate::{
    distribution::DistributionStrategy,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{Properties, Proptype},
    rays::Rays,
};
use nalgebra::Point3;
use std::collections::HashMap;
use std::fmt::Debug;
use uom::num_traits::Zero;
use uom::si::{
    f64::{Angle, Energy, Length},
    length::nanometer,
};
/// Generate a source node with a collinated beam.
///
/// This is a convenience functions, which generates a light source containing a hexapolar, collinear ray bundle at 1053 nm and a given energy.
///
/// # Errors
/// This functions returns an error if
///  - the given energy is < 0.0, Nan, or +inf.
pub fn create_round_collimated_ray_source(
    radius: Length,
    energy: Energy,
    nr_of_rings: u8,
) -> OpmResult<Source> {
    let rays = Rays::new_uniform_collimated(
        radius,
        Length::new::<nanometer>(1053.0),
        energy,
        &DistributionStrategy::Hexapolar { nr_of_rings },
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("collimated ray source", &light))
}
/// Create [`Source`] containing a line of rays along the y axis.
///
/// # Errors
///
/// This function will return an error if .
pub fn create_line_collimated_ray_source(
    size_y: Length,
    energy: Energy,
    nr_of_points_y: usize,
) -> OpmResult<Source> {
    let rays = Rays::new_uniform_collimated(
        size_y,
        Length::new::<nanometer>(1053.0),
        energy,
        &DistributionStrategy::Grid {
            nr_of_points_x: 1,
            nr_of_points_y,
        },
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("collimated ray source", &light))
}
/// Generate a node representing a points source on the optical axis with a given cone angle.
///
/// This is a convenience functions, which generates a light source containing a hexapolar, cone shaped ray bundle at 1053 nm and a given energy.
/// If the cone angle is zero, a ray bundle with a single ray along the optical axis - position (0.0,0.0,0.0) - is created.
/// # Errors
/// This functions returns an error if
///  - the given energy is < 0.0, Nan, or +inf.
///  - the given angle is < 0.0 degrees or >= 180.0 degrees.
pub fn create_point_ray_source(cone_angle: Angle, energy: Energy) -> OpmResult<Source> {
    let rays = Rays::new_hexapolar_point_source(
        Point3::new(Length::zero(), Length::zero(), Length::zero()),
        cone_angle,
        3,
        Length::new::<nanometer>(1000.0),
        energy,
    )?;
    let light = LightData::Geometric(rays);
    Ok(Source::new("point ray source", &light))
}
/// A general light source
///
/// Hence it has only one output port (out1) and no input ports. Source nodes usually are the first nodes of an [`OpticScenery`](crate::OpticScenery).
///
/// ## Optical Ports
///   - Inputs
///     - none
///   - Outputs
///     - `out1`
///
/// ## Properties
///   - `name`
///   - `light data`
///
/// **Note**: This node does not have the `inverted` property since it has only one output port.
pub struct Source {
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::new("source", "light source");
    props
        .create("light data", "data of the emitted light", None, None.into())
        .unwrap();
    let mut ports = OpticPorts::new();
    ports.create_output("out1").unwrap();
    props.set("apertures", ports.into()).unwrap();
    props
}

impl Default for Source {
    fn default() -> Self {
        Self {
            props: create_default_props(),
        }
    }
}
impl Source {
    /// Creates a new [`Source`].
    ///
    /// The light to be emitted from this source is defined in a [`LightData`] structure.
    ///
    /// # Panics
    /// Panics if [`Properties`] `name` can not be set
    ///
    /// ## Example
    ///
    /// ```rust
    /// use opossum::{
    /// lightdata::{DataEnergy, LightData},
    /// nodes::Source,
    /// spectrum_helper::create_he_ne_spec};
    ///
    /// let source=Source::new("My Source", &LightData::Energy(DataEnergy {spectrum: create_he_ne_spec(1.0).unwrap()}));
    /// ```
    #[must_use]
    pub fn new(name: &str, light: &LightData) -> Self {
        let mut props = create_default_props();
        props.set("name", name.into()).unwrap();
        props
            .set_unchecked("light data", Some(light.clone()).into())
            .unwrap();
        Self { props }
    }

    /// Sets the light data of this [`Source`]. The [`LightData`] provided here represents the input data of an `OpticScenery`.
    ///
    /// # Attributes
    /// * `light_data`: [`LightData`] that shall be set
    ///
    /// # Errors
    /// This function returns an error if the property "light data" can not be set
    pub fn set_light_data(&mut self, light_data: &LightData) -> OpmResult<()> {
        self.props
            .set("light data", Some(light_data.clone()).into())?;
        Ok(())
    }
}

impl Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let light_prop = self.props.get("light data").unwrap();
        let data = if let Proptype::LightData(data) = &light_prop {
            data
        } else {
            &None
        };
        match data {
            Some(data) => write!(f, "Source: {data}"),
            None => write!(f, "Source: no data"),
        }
    }
}
impl Optical for Source {
    fn analyze(
        &mut self,
        _incoming_edges: LightResult,
        _analyzer_type: &crate::analyzer::AnalyzerType,
    ) -> OpmResult<LightResult> {
        let light_prop = self.props.get("light data").unwrap();
        if let Proptype::LightData(Some(data)) = &light_prop {
            Ok(HashMap::from([("out1".into(), Some(data.clone()))]))
        } else {
            Err(OpossumError::Analysis("no light data defined".into()))
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn is_source(&self) -> bool {
        true
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        if name == "inverted" {
            let inverted = if let Proptype::Bool(inverted) = prop {
                inverted
            } else {
                false
            };
            if inverted {
                Err(OpossumError::Properties(
                    "Cannot change the inversion status of a source node!".into(),
                ))
            } else {
                Ok(())
            }
        } else {
            self.props.set(name, prop)
        }
    }
}

impl Dottable for Source {
    fn node_color(&self) -> &str {
        "slateblue"
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzer::AnalyzerType, lightdata::DataEnergy, spectrum_helper::create_he_ne_spec,
    };
    use approx::assert_abs_diff_eq;
    use assert_matches::assert_matches;
    use uom::si::{angle::degree, energy::joule, length::millimeter};
    #[test]
    fn test_create_collimated_ray_source() {
        assert!(create_round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(-0.1),
            3
        )
        .is_err());
        assert!(create_round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::NAN),
            3
        )
        .is_err());
        assert!(create_round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(f64::INFINITY),
            3
        )
        .is_err());
        assert!(create_round_collimated_ray_source(
            Length::new::<millimeter>(-0.1),
            Energy::new::<joule>(1.0),
            3
        )
        .is_err());
        let src = create_round_collimated_ray_source(Length::zero(), Energy::new::<joule>(1.0), 3)
            .unwrap();
        if let Proptype::LightData(light_data) = src.properties().get("light data").unwrap() {
            if let Some(LightData::Geometric(rays)) = light_data {
                assert_eq!(rays.nr_of_rays(), 1);
                assert_abs_diff_eq!(
                    rays.total_energy().get::<joule>(),
                    1.0,
                    epsilon = 10.0 * f64::EPSILON
                );
            } else {
                panic!("no LightData::Geometric found")
            }
        } else {
            panic!("property light data has wrong type");
        }
        let src = create_round_collimated_ray_source(
            Length::new::<millimeter>(1.0),
            Energy::new::<joule>(1.0),
            3,
        )
        .unwrap();
        if let Proptype::LightData(Some(LightData::Geometric(rays))) =
            src.properties().get("light data").unwrap()
        {
            assert_abs_diff_eq!(
                rays.total_energy().get::<joule>(),
                1.0,
                epsilon = 10.0 * f64::EPSILON
            );
            assert_eq!(rays.nr_of_rays(), 37);
        } else {
            panic!("error unpacking data");
        }
    }
    #[test]
    fn test_create_point_ray_source() {
        assert!(create_point_ray_source(Angle::new::<degree>(-0.1), Energy::zero()).is_err());
        assert!(create_point_ray_source(Angle::new::<degree>(180.0), Energy::zero()).is_err());
        assert!(create_point_ray_source(Angle::new::<degree>(190.0), Energy::zero()).is_err());
        let src = create_point_ray_source(Angle::zero(), Energy::new::<joule>(1.0)).unwrap();
        if let Ok(Proptype::LightData(Some(LightData::Geometric(rays)))) =
            src.properties().get("light data")
        {
            assert_abs_diff_eq!(
                rays.total_energy().get::<joule>(),
                1.0,
                epsilon = 10.0 * f64::EPSILON
            );
            assert_eq!(rays.nr_of_rays(), 1);
        } else {
            panic!("cannot unpack light data property");
        }
        let src =
            create_point_ray_source(Angle::new::<degree>(1.0), Energy::new::<joule>(1.0)).unwrap();
        if let Ok(Proptype::LightData(Some(LightData::Geometric(rays)))) =
            src.properties().get("light data")
        {
            assert_abs_diff_eq!(
                rays.total_energy().get::<joule>(),
                1.0,
                epsilon = 10.0 * f64::EPSILON
            );
            assert_eq!(rays.nr_of_rays(), 37);
        } else {
            panic!("cannot unpack light data property");
        }
    }
    #[test]
    fn default() {
        let node = Source::default();
        assert_eq!(node.properties().name().unwrap(), "source");
        assert_eq!(node.properties().node_type().unwrap(), "light source");
        if let Ok(Proptype::LightData(light_data)) = node.properties().get("light data") {
            assert_eq!(light_data, &None);
        } else {
            panic!("cannot unpack light data property");
        };
        assert_eq!(node.is_detector(), false);
        assert_eq!(node.properties().inverted().unwrap(), false);
        assert_eq!(node.node_color(), "slateblue");
        assert!(node.as_group().is_err());
    }
    #[test]
    fn new() {
        let source = Source::new("test", &LightData::Fourier);
        assert_eq!(source.properties().name().unwrap(), "test");
    }
    #[test]
    fn not_invertable() {
        let mut node = Source::default();
        assert!(node.set_property("inverted", false.into()).is_ok());
        assert!(node.set_property("inverted", true.into()).is_err());
    }
    #[test]
    fn ports() {
        let node = Source::default();
        assert!(node.ports().input_names().is_empty());
        assert_eq!(node.ports().output_names(), vec!["out1"]);
    }
    #[test]
    fn test_set_light_data() {
        let mut src = Source::default();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_eq!(light_data, &None);
        }
        src.set_light_data(&LightData::Fourier).unwrap();
        if let Ok(Proptype::LightData(light_data)) = src.properties().get("light data") {
            assert_matches!(light_data.clone().unwrap(), LightData::Fourier);
        }
    }
    #[test]
    fn analyze_empty() {
        let mut node = Source::default();
        let incoming_data: LightResult = LightResult::default();
        assert!(node.analyze(incoming_data, &AnalyzerType::Energy).is_err())
    }
    #[test]
    fn analyze_ok() {
        let light = LightData::Energy(DataEnergy {
            spectrum: create_he_ne_spec(1.0).unwrap(),
        });
        let mut node = Source::new("test", &light);
        let incoming_data: LightResult = LightResult::default();
        let output = node.analyze(incoming_data, &AnalyzerType::Energy);
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.contains_key("out1"));
        assert_eq!(output.len(), 1);
        let output = output.get("out1").unwrap();
        assert!(output.is_some());
        let output = output.clone().unwrap();
        assert_eq!(output, light);
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", Source::default()), "Source: no data");
        assert_eq!(
            format!("{:?}", Source::new("hallo", &LightData::Fourier)),
            "Source: No display defined for this type of LightData"
        );
    }
}
