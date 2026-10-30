//! Choosing the material a face of a component is drawn in.

use log::warn;
use opossum_core::{
    analyzers::Analyzable,
    core_optics::{Appearance, NodeAttrExt, SurfaceFinish},
    error::{OpmResult, OpossumError},
    geometry::body::Body,
};
use optoscene::Material;
use uom::si::f64::Length;

/// The refractive index used for a material that cannot state one at the asked wavelength.
///
/// Drawing the component in plain glass says more than leaving it out of the scene.
const FALLBACK_REFRACTIVE_INDEX: f64 = 1.5;

/// How opaque glass is drawn where nothing mirrors it.
///
/// A renderer such as three.js shows only opaque objects through transmissive glass, so a lens
/// behind another lens would vanish; drawn slightly see-through, it shows through instead (see the
/// handbook's chapter on the 3D view).
const GLASS_OPACITY: f32 = 0.7;

/// Map the [`Appearance`] of one face of a component to the closest [`optoscene::Material`]
/// preset.
///
/// Glass is the one material that needs more than the appearance, which only gives its tint and
/// how mirrored it is: see [`glass_of`].
///
/// # Arguments
///
/// - `node`: the component, named in errors
/// - `appearance`: what the face asks to be drawn in
/// - `body`: the volume the component's geometry encloses, if any
/// - `ior`: the refractive index of the medium that volume is filled with, which glass is drawn
///   with
///
/// # Returns
///
/// The [`Material`] a renderer should draw the face in.
///
/// # Errors
///
/// This function returns an error if glass is asked for but cannot be described (see
/// [`glass_of`]).
pub fn material_of(
    node: &dyn Analyzable,
    appearance: Appearance,
    body: Option<&dyn Body>,
    ior: f64,
) -> OpmResult<Material> {
    let [red, green, blue, alpha] = appearance.color;
    let rgb = [red, green, blue];
    Ok(match appearance.finish {
        SurfaceFinish::Reflective => Material::Mirror {
            color: rgb,
            roughness: appearance.roughness,
            metallic: appearance.metallic,
            opacity: alpha,
        },
        SurfaceFinish::Transmissive => Material::Translucent {
            color: appearance.color,
        },
        SurfaceFinish::Opaque => Material::Opaque {
            color: appearance.color,
            metallic: appearance.metallic,
            roughness: appearance.roughness,
        },
        // `Appearance` is renderer-agnostic and carries no thin-film parameters, so this
        // crate supplies sensible defaults here.
        SurfaceFinish::Iridescent => Material::Iridescent {
            color: rgb,
            roughness: appearance.roughness,
            iridescence: 1.0,
            iridescence_ior: 1.3,
            iridescence_thickness_min: 100.0,
            iridescence_thickness_max: 400.0,
        },
        SurfaceFinish::Glass => glass_of(node, appearance, body, ior)?,
    })
}

/// Return the refractive index of the medium a component's solid is filled with.
///
/// A medium that states no index at the wavelength is drawn as plain glass, with a warning.
///
/// # Arguments
///
/// - `node`: the component, named in errors and warnings
/// - `body`: the volume the component's geometry encloses
/// - `wavelength`: the wavelength to evaluate the refractive index at
///
/// # Errors
///
/// This function returns an error if the volume holds no single material.
pub fn refractive_index_of(
    node: &dyn Analyzable,
    body: &dyn Body,
    wavelength: Length,
) -> OpmResult<f64> {
    Ok(body
        .material()
        .ok_or_else(|| {
            OpossumError::Other(format!(
                "'{}' is filled with no single material to draw as glass",
                node.name()
            ))
        })?
        .refractive_index(wavelength)
        .unwrap_or_else(|e| {
            warn!(
                "'{}' states no refractive index at this wavelength ({e}), drawing it as plain \
                 glass",
                node.name()
            );
            FALLBACK_REFRACTIVE_INDEX
        }))
}

/// Describe the glass one face of a component is drawn in, as far as a renderer needs it.
///
/// Its thickness is the extent of the component's solid along its axis. Where nothing mirrors the
/// face, it is drawn at [`GLASS_OPACITY`]; the more it is mirrored, the more opaque it gets.
///
/// # Arguments
///
/// - `node`: the component, named in errors
/// - `appearance`: the tint of the glass and how mirrored the face is
/// - `body`: the volume the component's geometry encloses, if any
/// - `ior`: the refractive index of the medium that volume is filled with
///
/// # Errors
///
/// This function returns an error if the component's geometry encloses no volume, or if that
/// volume has no extent to measure a thickness from.
// A refractive index and a thickness in millimeters are nowhere near the range of an f32.
#[allow(clippy::cast_possible_truncation)]
fn glass_of(
    node: &dyn Analyzable,
    appearance: Appearance,
    body: Option<&dyn Body>,
    ior: f64,
) -> OpmResult<Material> {
    let body = body.ok_or_else(|| {
        OpossumError::Other(format!(
            "'{}' encloses no volume to draw as glass",
            node.name()
        ))
    })?;
    let thickness = body.bounding_box()?.z_range();
    let [red, green, blue, _] = appearance.color;
    Ok(Material::Glass {
        color: [red, green, blue],
        ior: ior as f32,
        thickness: (thickness.end - thickness.start).value as f32,
        metallic: appearance.metallic,
        opacity: (1.0 - GLASS_OPACITY).mul_add(appearance.metallic, GLASS_OPACITY),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        glb_of_node,
        test_helpers::{a_bit_of_everything, drawn_components, exported, glb_json},
    };
    use opossum_core::{
        analyzers::{AnalyzerType, RayTraceConfig},
        coatings::{CoatingConstantR, CoatingType},
        core_optics::{OpticNode, OpticNodeExt, node_attr::NodePositioning},
        degree,
        geometry::{Extruded, SurfaceGeometry},
        joule,
        light::lightdata::ray_data_builder::RayDataBuilder,
        material::default_reference_wavelength,
        millimeter, nanometer,
        nodes::{
            BeamSplitter, EnergyMeter, IdealFilter, Lens, NodeGroup, ReflectiveGrating, SourcePort,
            ThinMirror, round_collimated_ray_builder,
        },
        opm_document::OpmDocument,
        percent,
        utils::geom_transformation::Isometry,
    };

    /// A mirror is drawn reflective and a detector is drawn as a plain opaque surface - the two
    /// categories the plan calls for, told apart in the exported glTF by the material name
    /// `optoscene` gives each preset (`"mirror"`, `"opaque"`).
    #[test]
    fn a_mirror_and_a_detector_get_reflective_and_opaque_materials() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let mirror = scenery.add_node(ThinMirror::default())?;
        let detector = scenery.add_node(EnergyMeter::default())?;
        scenery.connect_nodes(source, "output_1", mirror, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(mirror, "output_1", detector, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let material_names: Vec<&str> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .map(|material| material["name"].as_str().expect("a named material"))
            .collect();
        assert!(
            material_names.contains(&"mirror"),
            "the mirror was not drawn reflective: {material_names:?}"
        );
        assert!(
            material_names.contains(&"opaque"),
            "the detector was not drawn as a plain opaque surface: {material_names:?}"
        );
        Ok(())
    }

    /// A filter lets light through, so it is drawn as a translucent surface (`optoscene`'s
    /// `"translucent"` preset) rather than a reflective or an opaque one.
    #[test]
    fn a_filter_is_drawn_translucent() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        let filter = scenery.add_node(IdealFilter::default())?;
        scenery.connect_nodes(source, "output_1", filter, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(source, RayDataBuilder::default());
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let material_names: Vec<&str> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .map(|material| material["name"].as_str().expect("a named material"))
            .collect();
        assert_eq!(material_names, ["translucent"]);
        Ok(())
    }
    /// A reflective grating overrides its default appearance to look iridescent, and the exported
    /// glTF must therefore carry the `KHR_materials_iridescence` extension.
    #[test]
    fn a_grating_is_drawn_iridescent() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let source = scenery.add_node(SourcePort::default())?;
        // The default grating (1740 lines/mm, order -1) diffracts 1000 nm light at its Littrow
        // angle; orient it exactly at Littrow (zero degrees off) for that wavelength so the order
        // is a valid, non-evanescent reflection and the grating is placed and drawn. The source
        // emits 1000 nm to match (`round_collimated_ray_builder`).
        let grating =
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))?;
        let grating = scenery.add_node(grating)?;
        scenery.connect_nodes(source, "output_1", grating, "input_1", millimeter!(50.0))?;
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        // The grating needs actual rays to define its diffraction up-direction; use a small
        // collimated bundle rather than the empty default builder.
        config.map_source(
            source,
            round_collimated_ray_builder(millimeter!(5.0), joule!(1.0), 1)?,
        );
        document.add_analyzer(AnalyzerType::RayTrace(config));

        let scene = exported(&document)?;
        let material_names: Vec<&str> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .map(|material| material["name"].as_str().expect("a named material"))
            .collect();
        assert!(
            material_names.contains(&"iridescent"),
            "the grating was not drawn iridescent: {material_names:?}"
        );
        assert!(
            scene["extensionsUsed"]
                .as_array()
                .expect("the extensions are listed")
                .iter()
                .any(|used| used == "KHR_materials_iridescence"),
            "KHR_materials_iridescence was not declared in extensionsUsed"
        );
        Ok(())
    }

    /// The materials a component's own file draws its patches in, patch by patch.
    fn patch_materials(node: &dyn Analyzable) -> OpmResult<Vec<serde_json::Value>> {
        let glb = glb_of_node(node, default_reference_wavelength())?
            .ok_or_else(|| OpossumError::Other("the component is not drawn".into()))?;
        let scene = glb_json(&glb);
        let mesh_index = drawn_components(&scene)[0]["mesh"]
            .as_u64()
            .expect("a drawn component has a mesh");
        Ok(scene["meshes"][mesh_index as usize]["primitives"]
            .as_array()
            .expect("a mesh has primitives")
            .iter()
            .map(|primitive| {
                let index = primitive["material"]
                    .as_u64()
                    .expect("a patch has a material");
                scene["materials"][index as usize].clone()
            })
            .collect())
    }
    /// How metallic and how opaque a material is drawn; one without an alpha mode is opaque.
    fn metallic_and_opacity(material: &serde_json::Value) -> (f64, f64) {
        let pbr = &material["pbrMetallicRoughness"];
        let metallic = pbr["metallicFactor"].as_f64().expect("a metallic factor");
        let opacity = material.get("alphaMode").map_or(1.0, |mode| {
            assert_eq!(mode, "BLEND");
            pbr["baseColorFactor"][3].as_f64().expect("an alpha")
        });
        (metallic, opacity)
    }
    /// Check that a material is drawn as metallic and as opaque as expected.
    fn assert_drawn_as(material: &serde_json::Value, metallic: f64, opacity: f64) {
        let (drawn_metallic, drawn_opacity) = metallic_and_opacity(material);
        assert!(
            (drawn_metallic - metallic).abs() < 1e-6 && (drawn_opacity - opacity).abs() < 1e-6,
            "drawn {drawn_metallic} metallic and {drawn_opacity} opaque instead of {metallic} and \
             {opacity}: {material}"
        );
    }
    /// A mirror is drawn as mirrored as its coating makes it: fully metallic at the mirror's own
    /// opacity by default, the pale pane it is without a coating that reflects.
    #[test]
    fn a_mirror_is_drawn_as_mirrored_as_its_coating_makes_it() -> OpmResult<()> {
        let mut mirror = ThinMirror::default();
        mirror.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        for material in patch_materials(&mirror)? {
            assert_drawn_as(&material, 1.0, 0.6);
        }
        mirror.set_coating(SurfaceGeometry::FACE, &CoatingType::IdealAR)?;
        for material in patch_materials(&mirror)? {
            assert_drawn_as(&material, 0.0, 0.3);
        }
        Ok(())
    }
    /// A beam splitter reflects half of the light, so it is drawn half mirrored.
    #[test]
    fn a_beam_splitter_is_drawn_half_mirrored() -> OpmResult<()> {
        let mut splitter = BeamSplitter::default();
        splitter.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        for material in patch_materials(&splitter)? {
            assert_drawn_as(&material, 0.5, 0.45);
        }
        Ok(())
    }
    /// Every face of a lens is drawn as slightly see-through glass, so that what lies behind it
    /// stays visible; a coating that mirrors one of its faces mirrors that face alone.
    #[test]
    fn a_mirror_coating_on_a_lens_mirrors_only_its_own_face() -> OpmResult<()> {
        let mut lens = Lens::default();
        lens.set_positioning(NodePositioning::Absolute(Isometry::identity()))?;
        let materials = patch_materials(&lens)?;
        assert_eq!(materials.len(), 3);
        for material in &materials {
            assert_eq!(material["name"], "glass");
            assert_drawn_as(material, 0.0, 0.7);
        }
        lens.set_coating(
            Extruded::REAR,
            &CoatingConstantR::new(percent!(100.0))?.into(),
        )?;
        // An uncoated face reflects by the index of the lens behind it: 4 % for n = 1.5.
        lens.set_coating(Extruded::FRONT, &CoatingType::Fresnel)?;
        let materials = patch_materials(&lens)?;
        // front, rear, lateral face
        assert_drawn_as(&materials[0], 0.04, 0.712);
        assert_drawn_as(&materials[1], 1.0, 1.0);
        assert_drawn_as(&materials[2], 0.0, 0.7);
        Ok(())
    }

    /// Glass is only glass to a renderer if it carries an index of refraction.
    ///
    /// `a_bit_of_everything`'s scene now also draws a detector, whose material is a plain opaque
    /// one and rightly carries no index of refraction at all - this only inspects the materials
    /// that state one in the first place, and separately pins down that there is at least one.
    #[test]
    fn glass_is_exported_with_its_refractive_index() -> OpmResult<()> {
        let document = a_bit_of_everything()?;
        let scene = exported(&document)?;
        let materials_with_ior: Vec<_> = scene["materials"]
            .as_array()
            .expect("a scene has materials")
            .iter()
            .filter(|material| !material["extensions"]["KHR_materials_ior"].is_null())
            .collect();
        assert!(
            !materials_with_ior.is_empty(),
            "the scene has three glass components, but none stated a refractive index"
        );
        for material in materials_with_ior {
            let ior = &material["extensions"]["KHR_materials_ior"]["ior"];
            assert!(
                ior.as_f64().is_some_and(|index| index >= 1.0),
                "a material was exported without a usable refractive index: {material}"
            );
        }
        assert!(
            scene["extensionsUsed"]
                .as_array()
                .expect("the extensions are listed")
                .iter()
                .any(|used| used == "KHR_materials_ior")
        );
        Ok(())
    }
}
