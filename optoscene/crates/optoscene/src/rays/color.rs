//! Wavelength to linear-RGB conversion for coloring ray lines.

/// Converts a vacuum wavelength (in meters) to an approximate linear RGB color.
///
/// Uses a piecewise-linear approximation of the visible spectrum from 380 nm to
/// 780 nm, with intensity falloff toward the edges. The result is treated as
/// linear RGB, because glTF color factors are linear.
///
/// # Arguments
/// * `lambda_m` — the wavelength in meters.
///
/// # Returns
/// `Some([r, g, b])` for wavelengths in `[380, 780] nm`, otherwise `None`.
#[allow(clippy::cast_possible_truncation)]
pub fn wavelength_to_rgb(lambda_m: f64) -> Option<[f32; 3]> {
    let nm = lambda_m * 1e9;
    if !(380.0..=780.0).contains(&nm) {
        return None;
    }

    let (mut r, mut g, mut b) = if nm < 440.0 {
        (-(nm - 440.0) / (440.0 - 380.0), 0.0, 1.0)
    } else if nm < 490.0 {
        (0.0, (nm - 440.0) / (490.0 - 440.0), 1.0)
    } else if nm < 510.0 {
        (0.0, 1.0, -(nm - 510.0) / (510.0 - 490.0))
    } else if nm < 580.0 {
        ((nm - 510.0) / (580.0 - 510.0), 1.0, 0.0)
    } else if nm < 645.0 {
        (1.0, -(nm - 645.0) / (645.0 - 580.0), 0.0)
    } else {
        (1.0, 0.0, 0.0)
    };

    let factor = if nm < 420.0 {
        0.3 + 0.7 * (nm - 380.0) / (420.0 - 380.0)
    } else if nm > 700.0 {
        0.3 + 0.7 * (780.0 - nm) / (780.0 - 700.0)
    } else {
        1.0
    };
    r *= factor;
    g *= factor;
    b *= factor;

    Some([r as f32, g as f32, b as f32])
}

#[cfg(test)]
mod tests {
    use super::wavelength_to_rgb;

    #[test]
    fn out_of_range_is_none() {
        assert!(wavelength_to_rgb(200e-9).is_none());
        assert!(wavelength_to_rgb(900e-9).is_none());
    }

    #[test]
    fn green_is_dominant_near_530_nm() {
        let [r, g, b] = wavelength_to_rgb(530e-9).unwrap();
        assert!(g > r && g > b);
    }
}
