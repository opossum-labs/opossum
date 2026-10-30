//! How each kind of component looks in the 3D view.

use opossum_core::{
    core_optics::{Appearance, SurfaceFinish},
    error::{OpmResult, OpossumError},
};

/// A reflective grating: a shiny surface showing interference colors.
const GRATING: Appearance = Appearance {
    color: [0.85, 0.85, 0.9, 1.0],
    roughness: 0.15,
    metallic: 1.0,
    finish: SurfaceFinish::Iridescent,
};

/// How every node type is drawn, by the name it is registered under, or `None` for a type that is
/// not drawn: one without a shape of its own, such as a group or a reference, or one that is only
/// traced at, such as a source.
const LOOKS: &[(&str, Option<Appearance>)] = &[
    ("lens", Some(Appearance::glass())),
    ("cylindric lens", Some(Appearance::glass())),
    ("wedge", Some(Appearance::glass())),
    ("bored mirror", Some(Appearance::glass())),
    ("mirror", Some(Appearance::mirror())),
    ("parabolic mirror", Some(Appearance::mirror())),
    ("beam splitter", Some(Appearance::mirror())),
    ("ideal filter", Some(Appearance::transmissive())),
    ("paraxial surface", Some(Appearance::transmissive())),
    ("energy meter", Some(Appearance::detector())),
    ("fluence detector", Some(Appearance::detector())),
    ("spectrometer", Some(Appearance::detector())),
    ("spot diagram", Some(Appearance::detector())),
    ("wavefront monitor", Some(Appearance::detector())),
    ("reflective grating", Some(GRATING)),
    ("dummy", None),
    ("group", None),
    ("reference", None),
    ("source port", None),
    ("ray propagation", None),
];

/// Return how nodes of a type are drawn.
///
/// # Arguments
///
/// - `node_type`: the name the node type is registered under
///
/// # Returns
///
/// The [`Appearance`] of the type, or `None` for a type that is not drawn.
///
/// # Errors
///
/// This function returns an error if no look is defined for the type.
pub fn appearance_of(node_type: &str) -> OpmResult<Option<Appearance>> {
    LOOKS
        .iter()
        .find(|(name, _)| *name == node_type)
        .map(|(_, look)| *look)
        .ok_or_else(|| {
            OpossumError::Other(format!(
                "no look is defined for the node type '{node_type}'"
            ))
        })
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::nodes::{create_node_ref, node_types};
    use std::collections::HashSet;

    /// Every node type that can be built has a look, so a new one cannot go undrawn unnoticed.
    #[test]
    fn every_registered_node_type_has_a_look() {
        let missing: Vec<&str> = node_types()
            .into_iter()
            .map(|(node_type, _)| node_type)
            .filter(|node_type| appearance_of(node_type).is_err())
            .collect();
        assert!(missing.is_empty(), "no look is defined for {missing:?}");
    }

    /// Every look belongs to a node type that exists, once: a renamed or removed type leaves no
    /// entry behind, and no entry is shadowed by an earlier one of the same name.
    #[test]
    fn every_look_names_a_registered_node_type() {
        let mut seen = HashSet::new();
        for (node_type, _) in LOOKS {
            assert!(
                create_node_ref(node_type).is_ok(),
                "a look is defined for '{node_type}', which is no node type"
            );
            assert!(seen.insert(node_type), "'{node_type}' has two looks");
        }
    }

    /// A name that is no node type has no look.
    #[test]
    fn a_name_that_is_no_node_type_has_no_look() {
        assert!(appearance_of("no such type").is_err());
    }

    /// Transitional: the table answers for every node type exactly as the node itself still does.
    #[test]
    fn the_table_agrees_with_every_node() -> OpmResult<()> {
        let node_types = node_types().into_iter().map(|(node_type, _)| node_type);
        for node_type in node_types.chain(["reference"]) {
            assert_eq!(
                appearance_of(node_type)?,
                create_node_ref(node_type)?.appearance(),
                "the table and the node disagree on how '{node_type}' looks"
            );
        }
        Ok(())
    }
}
