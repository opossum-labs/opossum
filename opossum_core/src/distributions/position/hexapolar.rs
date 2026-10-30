//! Circular, hexapolar distribution
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllPositive},
    millimeter, validated, validated_type,
};

use super::{PosDistType, PositionDistribution};
use nalgebra::{Point3, point};
use num_traits::Zero;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

/// Circular, hexapolar distribution
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct Hexapolar {
    #[validate(skip)]
    nr_of_rings: u8,
    radius: validated_type!(Length, AllPositive && AllFinite),
}
impl Hexapolar {
    /// Create a new [`Hexapolar`] distribution generator.
    ///
    /// If the given radius is zero and / or `nr_of_rings` is zero only the central point at (0,0) is generated.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the given `radius` is negative or not finite.
    pub fn new(radius: Length, nr_of_rings: u8) -> OpmResult<Self> {
        let mut hexapolar = Self::default();
        hexapolar.set_nr_of_rings(nr_of_rings);
        hexapolar.set_radius(radius)?;
        Ok(hexapolar)
    }

    /// Returns the radius of the hexapolar distribution.
    ///
    /// # Returns
    ///
    /// The radius as a `Length`.
    #[must_use]
    pub fn radius(&self) -> Length {
        *self.radius.get()
    }

    /// Returns the number of rings in the hexapolar distribution.
    ///
    /// # Returns
    ///
    /// The number of rings as a `u8`.
    #[must_use]
    pub const fn nr_of_rings(&self) -> u8 {
        self.nr_of_rings
    }

    /// Sets the radius of the hexapolar distribution.
    ///
    /// # Parameters
    ///
    /// * `radius` - The new radius as a `Length`.
    ///
    /// # Side Effects
    ///
    /// Updates the current radius.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_radius(&mut self, radius: Length) -> OpmResult<()> {
        self.radius.set(radius)?;
        Ok(())
    }

    /// Sets the number of rings in the hexapolar distribution.
    ///
    /// # Parameters
    ///
    /// * `nr_of_rings` - The new number of rings as a `u8`.
    ///
    /// # Side Effects
    ///
    /// Updates the current number of rings.
    pub const fn set_nr_of_rings(&mut self, nr_of_rings: u8) {
        self.nr_of_rings = nr_of_rings;
    }

    /// Returns the most rings a hexapolar distribution of at most `max_points` points can have.
    ///
    /// With `r` rings it has `1 + 3·r·(r + 1)` points: the center and `6·k` on ring `k`.
    ///
    /// # Arguments
    ///
    /// * `max_points` - the most points the distribution may have.
    ///
    /// # Returns
    ///
    /// The number of rings, zero if only the center fits.
    #[must_use]
    pub(super) fn max_rings_for(max_points: usize) -> u8 {
        let mut rings = 0_u8;
        while rings < u8::MAX {
            let next = usize::from(rings) + 1;
            if 1 + 3 * next * (next + 1) > max_points {
                break;
            }
            rings += 1;
        }
        rings
    }
}

impl Default for Hexapolar {
    fn default() -> Self {
        Self {
            nr_of_rings: 7,
            radius: validated!(millimeter!(5.), AllPositive && AllFinite).unwrap(),
        }
    }
}

impl PositionDistribution for Hexapolar {
    fn generate(&self) -> Vec<Point3<Length>> {
        let mut points: Vec<Point3<Length>> = Vec::new();
        // Add center point
        points.push(Point3::origin());
        // add rings if radius > 0.0
        if !self.radius().is_zero() {
            let radius_step = self.radius() / f64::from(self.nr_of_rings);
            for ring in 0..self.nr_of_rings {
                let radius = f64::from(ring + 1) * radius_step;
                let points_per_ring = 6 * u16::from(ring + 1);
                let angle_step = 2.0 * std::f64::consts::PI / f64::from(points_per_ring);
                for point_nr in 0..points_per_ring {
                    let point = (f64::from(point_nr) * angle_step).sin_cos();
                    points.push(point![radius * point.0, radius * point.1, Length::zero()]);
                }
            }
        }
        points
    }
    fn coarsened(&self, max_points: usize) -> OpmResult<PosDistType> {
        let mut coarse = *self;
        coarse.set_nr_of_rings(self.nr_of_rings.min(Self::max_rings_for(max_points)));
        Ok(coarse.into())
    }
}
impl From<Hexapolar> for super::PosDistType {
    fn from(dist: Hexapolar) -> Self {
        Self::Hexapolar(dist)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;
    #[test]
    fn new_wrong() {
        assert!(Hexapolar::new(millimeter!(-0.1), 1).is_err());
        assert!(Hexapolar::new(millimeter!(f64::NAN), 1).is_err());
        assert!(Hexapolar::new(millimeter!(f64::INFINITY), 1).is_err());
    }
    #[test]
    fn generate_one() -> OpmResult<()> {
        let g = Hexapolar::new(Length::zero(), 1)?;
        assert_eq!(g.generate().len(), 1);
        let g = Hexapolar::new(millimeter!(1.0), 0)?;
        assert_eq!(g.generate().len(), 1);
        Ok(())
    }
    #[test]
    fn generate() -> OpmResult<()> {
        let g = Hexapolar::new(millimeter!(1.0), 1)?;
        assert_eq!(g.generate().len(), 7);
        let g = Hexapolar::new(millimeter!(1.0), 2)?;
        assert_eq!(g.generate().len(), 19);
        Ok(())
    }
    #[test]
    fn generate_max() -> OpmResult<()> {
        let g = Hexapolar::new(millimeter!(1.0), u8::MAX)?;
        assert_eq!(g.generate().len(), 195841);
        Ok(())
    }
    #[test]
    fn generate_rounding() -> OpmResult<()> {
        let g = Hexapolar::new(millimeter!(1.0), 6)?;
        assert_eq!(g.generate().len(), 127);
        let g = Hexapolar::new(millimeter!(1.0), 7)?;
        assert_eq!(g.generate().len(), 169);
        Ok(())
    }
    #[test]
    fn max_rings_for_counts_the_center_and_whole_rings() {
        // 0, 1, 2, ... rings hold 1, 7, 19, ... points.
        for (max_points, rings) in [(0, 0), (6, 0), (7, 1), (18, 1), (19, 2), (100, 5)] {
            assert_eq!(Hexapolar::max_rings_for(max_points), rings, "{max_points}");
        }
        assert_eq!(Hexapolar::max_rings_for(usize::MAX), u8::MAX);
    }
    #[test]
    fn coarsened_keeps_fewer_rings_over_the_same_radius() -> OpmResult<()> {
        let fine = Hexapolar::new(millimeter!(5.0), 15)?;
        let PosDistType::Hexapolar(coarse) = fine.coarsened(100)? else {
            panic!("a coarsened hexapolar distribution is still hexapolar");
        };
        assert_eq!(coarse.nr_of_rings(), 5);
        assert_eq!(coarse.radius(), fine.radius());
        assert_eq!(coarse.generate().len(), 91);
        Ok(())
    }
}
