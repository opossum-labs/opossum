//! Drawing the paths ray bundles took as lines.

use nalgebra::Point3;
use opossum_core::{
    error::{OpmResult, OpossumError},
    light::Rays,
    nanometer,
};
use optoscene::{RayStyle, RayTrace, Scene, SceneOptions};
use uom::si::{f64::Length, length::meter};

/// The colour the optical axis is drawn in: a bright purple that stands apart from the wavelength
/// colours real rays are drawn in and from the grey and glass tones of the components.
pub const AXIS_COLOR: [f32; 4] = [183. / 255., 17. / 255., 1., 1.0];

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
/// - `color`: the colour of every line, or `None` to colour each by its wavelength
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
pub fn rays_glb(bundles: &[Rays], color: Option<[f32; 4]>, max_lines: usize) -> OpmResult<Vec<u8>> {
    let mut scene = Scene::new(SceneOptions {
        origin: Some(Point3::origin()),
        ..SceneOptions::default()
    });
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
                line_color: color.or_else(|| invisible_light_color(wavelength)),
                ..RayStyle::default()
            };
            scene
                .add_ray_trace(&trace, &style)
                .map_err(|e| OpossumError::Other(format!("the rays could not be drawn: {e}")))?;
        }
    }
    scene
        .to_glb()
        .map_err(|e| OpossumError::Other(format!("the scene could not be written: {e}")))
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
        assert!(rays_glb(&[], Some(AXIS_COLOR), 10)?.starts_with(b"glTF"));
        assert!(rays_glb(&[Rays::default()], None, 10)?.starts_with(b"glTF"));
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

        let glb = rays_glb(&[bundle], None, 10)?;
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

    /// An infrared laser is drawn red, not in the magenta that would make it look like the axis;
    /// visible light keeps its spectral colour.
    #[test]
    fn infrared_rays_are_drawn_red() -> OpmResult<()> {
        let color_of = |wavelength| -> OpmResult<serde_json::Value> {
            let mut ray = Ray::origin_along_z(wavelength, joule!(1.0))?;
            ray.propagate(millimeter!(10.0))?;
            let mut bundle = Rays::default();
            bundle.add_ray(ray);
            let scene = glb_json(&rays_glb(&[bundle], None, 10)?);
            Ok(scene["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"].clone())
        };
        let infrared = color_of(nanometer!(1053.0))?;
        let red: Vec<f64> = INFRARED_COLOR.iter().map(|c| f64::from(*c)).collect();
        let drawn: Vec<f64> = infrared
            .as_array()
            .expect("a line has a colour")
            .iter()
            .map(|c| c.as_f64().unwrap_or(f64::NAN))
            .collect();
        for (d, r) in drawn.iter().zip(&red) {
            assert!((d - r).abs() < 1e-6, "infrared drawn as {infrared}");
        }
        assert_ne!(
            color_of(nanometer!(550.0))?,
            infrared,
            "visible light keeps its colour"
        );
        Ok(())
    }

    /// The same rays give the same bytes, so a viewer can tell an unchanged file from a new one.
    #[test]
    fn the_same_rays_give_the_same_file() -> OpmResult<()> {
        let axes = two_lenses(millimeter!(100.0))?.positioning_rays(None)?;
        assert!(!axes.is_empty(), "the model has an open output");
        assert_eq!(
            rays_glb(&axes, Some(AXIS_COLOR), 10)?,
            rays_glb(&axes, Some(AXIS_COLOR), 10)?
        );
        Ok(())
    }
}
