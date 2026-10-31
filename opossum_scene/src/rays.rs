//! Drawing the paths ray bundles took as lines.

use std::ops::Range;

use nalgebra::Point3;
use opossum_core::{
    error::{OpmResult, OpossumError},
    light::Rays,
    nanometer,
    types::api_types::SceneSpectrum,
    utils::{to_f64, try_f64_to_usize},
};
use optoscene::{RayStyle, RayTrace};
use uom::si::{f64::Length, length::meter};

use crate::{glb, pinned_scene};

/// The colour the optical axis is drawn in: a bright purple that stands apart from the wavelength
/// colours real rays are drawn in and from the grey and glass tones of the components.
pub const AXIS_COLOR: [f32; 4] = [183. / 255., 17. / 255., 1., 1.0];

/// The false colors light of several wavelengths is drawn in, from the shortest wavelength to the
/// longest, as sRGB: blue, cyan, green, yellow and red. Violet is left out: the axis is drawn in it.
const SPECTRUM_COLORS: [[f32; 3]; 5] = [
    [0.15, 0.35, 1.0],
    [0.0, 0.8, 0.9],
    [0.2, 0.8, 0.2],
    [1.0, 0.8, 0.0],
    [0.9, 0.1, 0.05],
];

/// How [`rays_glb`] colors the lines it draws.
#[derive(Debug, Clone, PartialEq)]
pub enum LineColors {
    /// Every line in the one color given, as the optical axis is drawn.
    Fixed([f32; 4]),
    /// Each line after its wavelength, within the range of wavelengths the model's sources emit.
    ///
    /// A range of more than one wavelength is spread over false colors from blue to red, so that
    /// neighboring wavelengths stand apart even where the eye could not see or tell them apart.
    /// Light of a single wavelength, or with no range given, is drawn in the color of its
    /// wavelength, with infrared in red and ultraviolet in violet.
    Wavelength(Option<Range<Length>>),
}

impl LineColors {
    /// Returns the false colors the lines are drawn in, for a legend.
    ///
    /// # Returns
    ///
    /// The range of wavelengths and the colors it is spread over, or `None` if the lines are not
    /// drawn in false colors.
    #[must_use]
    pub fn legend(&self) -> Option<SceneSpectrum> {
        let range = self.false_color_range()?;
        Some(SceneSpectrum {
            shortest_nm: range.start.get::<uom::si::length::nanometer>(),
            longest_nm: range.end.get::<uom::si::length::nanometer>(),
            colors: SPECTRUM_COLORS.to_vec(),
        })
    }

    /// Returns the range of wavelengths spread over the false colors, if there is one to spread.
    fn false_color_range(&self) -> Option<&Range<Length>> {
        match self {
            Self::Wavelength(Some(range)) if range.start < range.end => Some(range),
            _ => None,
        }
    }

    /// Returns the color of the lines of one wavelength.
    ///
    /// # Arguments
    ///
    /// - `wavelength`: the vacuum wavelength of the light
    ///
    /// # Returns
    ///
    /// The color, or `None` for visible light that `optoscene` colors after its wavelength.
    fn of(&self, wavelength: Length) -> Option<[f32; 4]> {
        if let Self::Fixed(color) = self {
            return Some(*color);
        }
        let Some(range) = self.false_color_range() else {
            return invisible_light_color(wavelength);
        };
        Some(false_color(
            ((wavelength - range.start) / (range.end - range.start)).value,
        ))
    }
}

/// Returns the false color at `t` along the range of wavelengths drawn.
///
/// The color is interpolated between the neighboring false colors and made linear, as glTF wants
/// its colors.
///
/// # Arguments
///
/// - `t`: 0 for the shortest wavelength, 1 for the longest; anything beyond is clamped
///
/// # Returns
///
/// The linear RGBA color.
fn false_color(t: f64) -> [f32; 4] {
    let segments = SPECTRUM_COLORS.len() - 1;
    let position = t.clamp(0.0, 1.0) * to_f64(segments);
    // The last segment also takes t = 1.
    let segment = try_f64_to_usize(position).unwrap_or(0).min(segments - 1);
    #[allow(clippy::cast_possible_truncation)]
    let fraction = (position - to_f64(segment)) as f32;
    let mut color = [0.0, 0.0, 0.0, 1.0];
    if let Some([from, to]) = SPECTRUM_COLORS.windows(2).nth(segment) {
        for ((channel, from), to) in color.iter_mut().zip(from).zip(to) {
            *channel = srgb_to_linear((to - from).mul_add(fraction, *from));
        }
    }
    color
}

/// Turns an sRGB color channel into the linear one glTF wants.
///
/// # Arguments
///
/// - `channel`: the sRGB value, from 0 to 1
///
/// # Returns
///
/// The linear value, from 0 to 1.
fn srgb_to_linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// Build the glTF binary of ray bundles, drawn as lines along the paths the rays took.
///
/// Every ray carries its position history from its source on, so a bundle that left a model
/// draws the whole way it came. A bundle is split by wavelength first, so that each part can be
/// drawn in its own colour. Rays that were stopped on the way - by an aperture, say - are drawn
/// up to where they stopped rather than left out: where light is lost is part of the picture.
///
/// The rays are already in world coordinates, so the scene is pinned at the coordinate origin
/// like [`glb_of_node`](crate::glb_of_node), and a viewer places the file where the components are
/// without moving it.
///
/// # Arguments
///
/// - `bundles`: the ray bundles to draw
/// - `colors`: how the lines are colored
/// - `max_lines`: how many rays of each wavelength of a bundle are drawn at most
///
/// # Returns
///
/// The GLB bytes of a scene holding the lines. It is empty but still a valid file if no ray
/// travelled anywhere.
///
/// # Errors
///
/// This function returns an error if a bundle cannot be split by wavelength, or if the scene
/// cannot be built or written.
pub fn rays_glb(bundles: &[Rays], colors: &LineColors, max_lines: usize) -> OpmResult<Vec<u8>> {
    let mut scene = pinned_scene();
    for (bundle_nr, bundle) in bundles.iter().enumerate() {
        if bundle.nr_of_rays(false) == 0 {
            continue;
        }
        let (parts, wavelengths) = bundle.split_ray_bundle_by_wavelength(nanometer!(1.0), false)?;
        for (part_nr, (part, wavelength)) in parts.iter().zip(wavelengths).enumerate() {
            let trace = ray_trace_of(part, format!("rays/{bundle_nr}/{part_nr}"), wavelength);
            // A path needs two points to be drawn at all.
            if trace.stations.len() < 2 {
                continue;
            }
            let style = RayStyle {
                max_lines,
                line_color: colors.of(wavelength),
                ..RayStyle::default()
            };
            scene
                .add_ray_trace(&trace, &style)
                .map_err(|e| OpossumError::Other(format!("the rays could not be drawn: {e}")))?;
        }
    }
    glb(&scene)
}

/// The colour infrared light is drawn in: a deep red, next to the visible end of the spectrum it
/// lies beyond.
const INFRARED_COLOR: [f32; 4] = [0.8, 0.05, 0.05, 1.0];
/// The colour ultraviolet light is drawn in: a violet, next to the visible end of the spectrum it
/// lies beyond.
const ULTRAVIOLET_COLOR: [f32; 4] = [0.45, 0.1, 0.9, 1.0];

/// The colour light of a wavelength the eye cannot see is drawn in.
///
/// Visible light is coloured by `optoscene` after its wavelength. Beyond the visible range it
/// falls back to one magenta for everything, which would make an infrared laser look like the
/// optical axis; so infrared and ultraviolet light get colours of their own here, taken from the
/// end of the spectrum each lies beyond.
///
/// # Arguments
///
/// - `wavelength`: the vacuum wavelength of the light
///
/// # Returns
///
/// The colour for infrared or ultraviolet light, or `None` for visible light.
fn invisible_light_color(wavelength: Length) -> Option<[f32; 4]> {
    let nm = wavelength.get::<uom::si::length::nanometer>();
    if nm > 780.0 {
        Some(INFRARED_COLOR)
    } else if nm < 380.0 {
        Some(ULTRAVIOLET_COLOR)
    } else {
        None
    }
}

/// Lay the position histories of a bundle out as the stations of an `optoscene` ray trace.
///
/// Station `k` holds the `k`-th point of every ray's path. Paths are not equally long - a ray
/// stopped early has fewer points - so a ray that has none left at a station is missing there,
/// which is how `optoscene` tells a lost ray.
///
/// # Arguments
///
/// - `rays`: the rays whose paths to lay out
/// - `uid`: the identifier the trace is drawn under
/// - `wavelength`: the wavelength the lines are coloured by
///
/// # Returns
///
/// The ray trace, in metres.
fn ray_trace_of(rays: &Rays, uid: String, wavelength: Length) -> RayTrace {
    let paths: Vec<Vec<Point3<f64>>> = rays
        .iter()
        .map(|ray| {
            ray.position_history_with_current()
                .row_iter()
                .map(|point| {
                    Point3::new(
                        point[0].get::<meter>(),
                        point[1].get::<meter>(),
                        point[2].get::<meter>(),
                    )
                })
                .collect()
        })
        .collect();
    let nr_of_stations = paths.iter().map(Vec::len).max().unwrap_or(0);
    RayTrace {
        uid,
        stations: (0..nr_of_stations)
            .map(|k| paths.iter().map(|path| path.get(k).copied()).collect())
            .collect(),
        wavelength: Some(wavelength.get::<meter>()),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::{glb_json, two_lenses};
    use opossum_core::{joule, light::Ray, millimeter};

    /// Nothing to draw still makes a file, so a viewer can show "no rays" like any other state.
    #[test]
    fn no_rays_are_still_a_file() -> OpmResult<()> {
        assert!(rays_glb(&[], &LineColors::Fixed(AXIS_COLOR), 10)?.starts_with(b"glTF"));
        assert!(
            rays_glb(&[Rays::default()], &LineColors::Wavelength(None), 10)?.starts_with(b"glTF")
        );
        Ok(())
    }

    /// Rays of one bundle whose paths are not equally long - one stopped early - are drawn up to
    /// where each of them got, as one set of line segments per path leg.
    #[test]
    fn rays_are_drawn_as_line_segments_along_their_paths() -> OpmResult<()> {
        let mut stopped_early = Ray::origin_along_z(nanometer!(1000.0), joule!(1.0))?;
        let mut went_on = stopped_early.clone();
        stopped_early.propagate(millimeter!(10.0))?;
        went_on.propagate(millimeter!(10.0))?;
        went_on.propagate(millimeter!(10.0))?;
        let mut bundle = Rays::default();
        bundle.add_ray(stopped_early);
        bundle.add_ray(went_on);

        let glb = rays_glb(&[bundle], &LineColors::Wavelength(None), 10)?;
        let scene = glb_json(&glb);
        let primitive = &scene["meshes"][0]["primitives"][0];
        assert_eq!(primitive["mode"], 1, "drawn as LINES");
        let indices = primitive["indices"].as_u64().expect("lines are indexed");
        let index_count =
            &scene["accessors"][usize::try_from(indices).unwrap_or(usize::MAX)]["count"];
        assert_eq!(
            index_count,
            2 * (1 + 2),
            "one leg for the first ray, two for the second"
        );
        Ok(())
    }

    /// The colors the lines of rays at the given wavelengths are drawn in, one per wavelength.
    fn drawn_colors(wavelengths: &[Length], colors: &LineColors) -> OpmResult<Vec<Vec<f64>>> {
        let mut bundle = Rays::default();
        for wavelength in wavelengths {
            let mut ray = Ray::origin_along_z(*wavelength, joule!(1.0))?;
            ray.propagate(millimeter!(10.0))?;
            bundle.add_ray(ray);
        }
        let scene = glb_json(&rays_glb(&[bundle], colors, 10)?);
        Ok(scene["materials"]
            .as_array()
            .expect("lines have materials")
            .iter()
            .map(|material| {
                material["pbrMetallicRoughness"]["baseColorFactor"]
                    .as_array()
                    .expect("a line has a color")
                    .iter()
                    .map(|c| c.as_f64().unwrap_or(f64::NAN))
                    .collect()
            })
            .collect())
    }

    /// Whether two colors agree within what `f32` keeps.
    fn same_color(drawn: &[f64], expected: [f32; 4]) -> bool {
        drawn
            .iter()
            .zip(expected)
            .all(|(d, e)| (d - f64::from(e)).abs() < 1e-6)
    }

    /// An infrared laser is drawn red, not in the magenta that would make it look like the axis;
    /// visible light keeps its spectral colour. So does light of one wavelength given as a range.
    #[test]
    fn infrared_rays_are_drawn_red() -> OpmResult<()> {
        let infrared = nanometer!(1053.0);
        for colors in [
            LineColors::Wavelength(None),
            LineColors::Wavelength(Some(infrared..infrared)),
        ] {
            let drawn = drawn_colors(&[infrared], &colors)?;
            assert!(
                same_color(&drawn[0], INFRARED_COLOR),
                "{colors:?}: {drawn:?}"
            );
            assert_ne!(
                drawn_colors(&[nanometer!(550.0)], &colors)?,
                drawn,
                "visible light keeps its colour"
            );
        }
        Ok(())
    }

    /// Over a range of wavelengths, its ends are drawn in the first and last false color and what
    /// lies between in another, even where all of it is infrared.
    #[test]
    fn a_range_of_wavelengths_is_drawn_in_false_colors() -> OpmResult<()> {
        let colors = LineColors::Wavelength(Some(nanometer!(1000.0)..nanometer!(1100.0)));

        let drawn = drawn_colors(
            &[nanometer!(1000.0), nanometer!(1050.0), nanometer!(1100.0)],
            &colors,
        )?;

        assert_eq!(drawn.len(), 3, "one material per wavelength");
        assert!(same_color(&drawn[0], false_color(0.0)));
        assert!(same_color(&drawn[2], false_color(1.0)));
        assert_ne!(drawn[1], drawn[0]);
        assert_ne!(drawn[1], drawn[2]);
        Ok(())
    }

    /// The ends of the false colors are blue and red, made linear for glTF.
    #[test]
    fn the_false_colors_run_from_blue_to_red() {
        let [r, g, b, _] = false_color(0.0);
        assert!(b > r && b > g, "blue at the start: {r} {g} {b}");
        let [r, g, b, _] = false_color(1.0);
        assert!(r > g && r > b, "red at the end: {r} {g} {b}");
        assert_eq!(false_color(-1.0), false_color(0.0), "clamped below");
        assert_eq!(false_color(2.0), false_color(1.0), "clamped above");
        assert!((srgb_to_linear(0.5) - 0.214).abs() < 1e-3, "the sRGB curve");
    }

    /// Only lines drawn in false colors have a legend, and it names their range.
    #[test]
    fn only_false_colors_have_a_legend() {
        let legend = LineColors::Wavelength(Some(nanometer!(1000.0)..nanometer!(1100.0))).legend();
        assert_eq!(
            legend,
            Some(SceneSpectrum {
                shortest_nm: 1000.0,
                longest_nm: 1100.0,
                colors: SPECTRUM_COLORS.to_vec(),
            })
        );
        let one = nanometer!(1053.0);
        assert_eq!(LineColors::Wavelength(Some(one..one)).legend(), None);
        assert_eq!(LineColors::Wavelength(None).legend(), None);
        assert_eq!(LineColors::Fixed(AXIS_COLOR).legend(), None);
    }

    /// The same rays give the same bytes, so a viewer can tell an unchanged file from a new one.
    #[test]
    fn the_same_rays_give_the_same_file() -> OpmResult<()> {
        let axes = two_lenses(millimeter!(100.0))?.positioning_rays(None)?;
        assert!(!axes.is_empty(), "the model has an open output");
        assert_eq!(
            rays_glb(&axes, &LineColors::Fixed(AXIS_COLOR), 10)?,
            rays_glb(&axes, &LineColors::Fixed(AXIS_COLOR), 10)?
        );
        Ok(())
    }
}
