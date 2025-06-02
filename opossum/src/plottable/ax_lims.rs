//! This module contains the [`AxLims`] struct, which is used to define the axis limits of a plot.
use approx::{RelativeEq, abs_diff_ne};
use log::warn;
use nalgebra::{DVector, DVectorView};

use crate::{
    error::{OpmResult, OpossumError},
    utils::filter_data::filter_nan_infinite,
};

/// Struct that holds the maximum and minimum values of an axis
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct AxLims {
    /// minimum value of the axis
    pub min: f64,
    /// maximum value of the axis
    pub max: f64,
}

impl TryFrom<Option<(f64, f64)>> for AxLims {
    type Error = OpossumError;

    fn try_from(value: Option<(f64, f64)>) -> OpmResult<Self> {
        if let Some((min, max)) = value {
            Self::new(min, max).map_or_else(
                || {
                    Err(OpossumError::Other(format!(
                        "Cannot create AxLim from values (min:{min}, max:{max})"
                    )))
                },
                Ok,
            )
        } else {
            Err(OpossumError::Other("Cannot create AxLim from None".into()))
        }
    }
}

impl AxLims {
    ///Creates a new [`AxLims`] struct
    /// # Attributes
    /// -`min`: minimum value of the ax limit
    /// -`max`: maximum value of the ax limit
    ///
    /// # Returns
    /// This function retuns Some([`AxLims`]) or None if the chosen minimum or maximum valus is NaN or infinite
    #[must_use]
    pub fn new(min: f64, max: f64) -> Option<Self> {
        let axlim = Self { min, max };
        if axlim.check_validity() {
            Some(axlim)
        } else {
            warn!("Invalid axis limits. Must be finite and min < max. Use default");
            None
        }
    }

    ///Creates a new [`AxLims`] struct from a provided `DVector` filtering out all non finite values
    /// # Attributes
    /// -`dat_vec`: data vector
    ///
    /// # Returns
    /// This function retuns Some([`AxLims`]) or None if non of the dvector entries is finite
    #[must_use]
    pub fn finite_from_dvector(dat_vec: &DVectorView<'_, f64>) -> Option<Self> {
        let filtered_data = DVector::from_vec(filter_nan_infinite(dat_vec.as_slice()));
        if filtered_data.len() < 2 {
            warn!(
                "Length of input data after filtering out non-finite values is below 2! Useful Axlims cannot be returned! AxLimit is set to None!"
            );
            None
        } else {
            let axlim = Self {
                min: filtered_data.min(),
                max: filtered_data.max(),
            };
            if axlim.check_validity() {
                Some(axlim)
            } else {
                warn!("Invalid axis limits. Must be finite and min < max. Use default");
                None
            }
        }
    }

    /// Checks the validity of the delivered min and max values and returns a true if it is valid, false otherwise
    #[must_use]
    pub fn check_validity(self) -> bool {
        self.max.is_finite()
            && self.min.is_finite()
            && abs_diff_ne!(self.max, self.min)
            && self.max > self.min
    }

    /// Shifts the minimum and the maximum to lower and higher values, respectively.
    /// The range expands by the `expansion_factor`, therefore, each limit is shifted by `range` * (`expansion_ratio`-1.)/2.
    /// # Attributes
    /// -`ratio`: relative extension of the range. must be positive, non-zero, not NAN and finite
    /// # Errors
    /// This function errors if the expansion ration is neither positive nor normal
    pub fn expand_lim_range_by_factor(&mut self, expansion_factor: f64) {
        if expansion_factor.is_normal() && expansion_factor.is_sign_positive() {
            let range = self.max - self.min;
            self.max += range * (expansion_factor - 1.) / 2.;
            self.min -= range * (expansion_factor - 1.) / 2.;
        } else {
            warn!("Cannot expand ax limits! Expansion factor must be normal and positive!");
        }
    }

    /// This function creates an [`AxLims`] struct from the provided `min` and `max` values
    /// # Attributes
    /// - `min`: minimum value for the ax limit
    /// - `max`: maximum value for the ax limit
    /// # Returns
    /// If the minimum and maximum value are chosen such that min < max and both finite, the function returns Some([`AxLims`]) with these limits
    /// If these criteria are not fulfilled, the values are changed accordingly to provide valid axlims. If for some reason, these values are still no okay, teh function returns None
    #[must_use]
    pub fn create_useful_axlims(min_in: f64, max_in: f64) -> Option<Self> {
        if !min_in.is_finite() && !max_in.is_finite() {
            return Self::new(-0.5, 0.5);
        }

        let (min, max) = if !min_in.is_finite() {
            (max_in, max_in)
        } else if !max_in.is_finite() {
            (min_in, min_in)
        } else {
            (min_in, max_in)
        };

        let (mut min, mut max) = if max < min { (max, min) } else { (min, max) };

        let mut ax_range = max - min;

        //check if minimum and maximum values are approximately equal. if so, take the max value as range
        if max.relative_eq(&min, f64::EPSILON, f64::EPSILON) {
            ax_range = max.abs();
            min = max - ax_range / 2.;
            max += ax_range * 0.5;
        }

        //check if for some reason maximum is 0, then set it to 1, so that the axis spans at least some distance
        if ax_range < f64::EPSILON {
            max = 0.5;
            min = -0.5;
        }
        Self::new(min, max)
    }

    /// Joins the minimum and maximum values of this [`AxLims`] struct with another [`AxLims`] struct, such that the maximum and minimum of both structs are used
    /// # Attributes
    /// - `ax_lim`: [`AxLims`] struct to integrate
    pub fn join(&mut self, ax_lim: Self) {
        if self.min > ax_lim.min {
            self.min = ax_lim.min;
        }

        if self.max < ax_lim.max {
            self.max = ax_lim.max;
        }
    }

    /// Joins the minimum and maximum values of this [`AxLims`] struct with an [`AxLims`] struct Option, such that the maximum and minimum of both structs are used
    /// Convenience function to join plotbounds
    /// # Attributes
    /// - `ax_lim_opt`: [`AxLims`] struct Option to join
    pub fn join_opt(&mut self, ax_lim_opt: Option<Self>) {
        if let Some(ax_lim) = ax_lim_opt {
            self.join(ax_lim);
        }
    }
}
#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use crate::utils::test_helper::test_helper::check_logs;

    use super::*;
    #[test]
    fn check_ax_lim_validity_valid() {
        assert!(AxLims { min: 0., max: 1. }.check_validity());
        assert!(
            AxLims {
                min: -10.,
                max: 10.
            }
            .check_validity()
        );
    }
    #[test]
    fn check_ax_lim_validity_nan() {
        assert!(
            !AxLims {
                min: f64::NAN,
                max: 1.
            }
            .check_validity()
        );
        assert!(
            !AxLims {
                min: 0.,
                max: f64::NAN
            }
            .check_validity()
        );
    }
    #[test]
    fn check_ax_lim_validity_equal() {
        assert!(!AxLims { min: 1., max: 1. }.check_validity());
        assert!(!AxLims { min: -1., max: -1. }.check_validity());
        assert!(
            !AxLims {
                min: 1e20,
                max: 1e20
            }
            .check_validity()
        );
        assert!(
            !AxLims {
                min: -1e20,
                max: -1e20
            }
            .check_validity()
        );
    }
    #[test]
    fn check_ax_lim_validity_max_smaller() {
        assert!(!AxLims { min: 1., max: 0. }.check_validity());
    }
    #[test]
    fn check_ax_lim_validity_infinite() {
        assert!(
            !AxLims {
                min: f64::INFINITY,
                max: 1.
            }
            .check_validity()
        );
        assert!(
            !AxLims {
                min: 0.,
                max: f64::INFINITY
            }
            .check_validity()
        );
        assert!(
            !AxLims {
                min: -f64::INFINITY,
                max: 1.
            }
            .check_validity()
        );
        assert!(
            !AxLims {
                min: 0.,
                max: -f64::INFINITY
            }
            .check_validity()
        );
    }
    #[test]
    fn axlim_new() {
        assert!(AxLims::new(-10., 10.).is_some());
        assert!(AxLims::new(0., f64::NAN).is_none());

        assert!((AxLims::new(-10., 10.).unwrap().min + 10.).abs() < f64::EPSILON);
        assert!((AxLims::new(-10., 10.).unwrap().max - 10.).abs() < f64::EPSILON);
    }
    #[test]
    fn axlim_expand() {
        let mut axlim = AxLims::new(-10., 10.).unwrap();
        let _ = axlim.expand_lim_range_by_factor(1.2);

        assert!((axlim.min + 12.).abs() < f64::EPSILON);
        assert!((axlim.max - 12.).abs() < f64::EPSILON);

        testing_logger::setup();
        axlim.expand_lim_range_by_factor(-1.);
        axlim.expand_lim_range_by_factor(f64::NAN);
        axlim.expand_lim_range_by_factor(f64::INFINITY);
        axlim.expand_lim_range_by_factor(0.);

        check_logs(
            log::Level::Warn,
            vec![
                "Cannot expand ax limits! Expansion factor must be normal and positive!",
                "Cannot expand ax limits! Expansion factor must be normal and positive!",
                "Cannot expand ax limits! Expansion factor must be normal and positive!",
                "Cannot expand ax limits! Expansion factor must be normal and positive!",
            ],
        );
    }
    #[test]
    fn create_useful_axlims_test() {
        let axlim = AxLims::create_useful_axlims(0., 10.).unwrap();
        assert_relative_eq!(axlim.min, 0.);
        assert_relative_eq!(axlim.max, 10.);

        let axlim = AxLims::create_useful_axlims(10., 10.).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);

        let axlim = AxLims::create_useful_axlims(0., 0.).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(f64::NAN, 0.).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(f64::NAN, 10.).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);

        let axlim = AxLims::create_useful_axlims(0., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(-10., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, -15.);
        assert_relative_eq!(axlim.max, -5.);

        let axlim = AxLims::create_useful_axlims(10., -10.).unwrap();
        assert_relative_eq!(axlim.min, -10.);
        assert_relative_eq!(axlim.max, 10.);

        let axlim = AxLims::create_useful_axlims(10., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);
    }
}
