#![warn(missing_docs)]
//! Data structures containing the light information flowing between [`OpticNode`s](crate::optic_node::OpticNode).

pub mod energy_data_builder;
pub mod light_data_builder;
pub mod ray_data_builder;
use crate::{joule, rays::Rays, spectrum::Spectrum};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uom::{fmt::DisplayStyle::Abbreviation, si::energy::joule};

/// Data structure defining the light properties.
///
/// The actuals data type used depends on the [`AnalyzerType`](crate::analyzers::AnalyzerType).
/// For example, an energy analysis ([`LightData::Energy`]) only
/// contains a [`Spectrum`] information, while a geometric analysis ([`LightData::Geometric`]) constains a set of optical
/// ray data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LightData {
    /// data type used for energy analysis.
    Energy(Spectrum),
    /// data type used for geometric optics analysis (ray tracing)
    Geometric(Rays),
    /// data type used for ghost focus analysis (back- and forth ray-tracing)
    GhostFocus(Vec<Rays>),
    /// placeholder value for future Fourier optics analysis, nothing implementd yet.
    Fourier,
}
impl Display for LightData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Energy(s) => {
                write!(
                    f,
                    "Energy: {}",
                    joule!(s.total_energy()).into_format_args(joule, Abbreviation)
                )
            }
            _ => write!(f, "No display defined for this type of LightData"),
        }
    }
}
#[cfg(test)]
mod test {
    use crate::{
        lightdata::light_data_builder::LightDataBuilder, properties::Proptype,
        spectrum_helper::create_visible_spec,
    };

    use super::*;
    use assert_matches::assert_matches;
    #[test]
    fn display_unknown() {
        assert_eq!(
            format!("{}", LightData::Fourier),
            "No display defined for this type of LightData"
        );
    }
    #[test]
    fn display_energy() {
        let ld = LightData::Energy(create_visible_spec());
        assert_eq!(format!("{ld}"), "Energy: 0 J");
    }
    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", LightData::Fourier), "Fourier");
    }
    #[test]
    fn from() {
        let ld = Proptype::from(Some(LightDataBuilder::Fourier));
        assert_matches!(ld, Proptype::LightDataBuilder(_));
    }
}
