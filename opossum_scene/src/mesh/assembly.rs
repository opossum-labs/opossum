//! Drawing several solids sharing interfaces.

use opossum_core::{
    apertures::Outline,
    error::OpmResult,
    geometry::{Assembly, FaceIndex},
};

use super::{Meshable, surface_mesh::FacePatch};

/// An assembly is drawn as its parts, part after part, each patch on the face its part's face is
/// numbered as in the assembly.
///
/// # Errors
///
/// This function returns an error if a part cannot be meshed.
impl Meshable for Assembly {
    fn mesh(&self, segments: usize, unbounded: &Outline) -> OpmResult<Vec<FacePatch>> {
        let mut patches = Vec::new();
        for (first_face, part) in self.parts() {
            patches.extend(part.mesh(segments, unbounded)?.into_iter().map(|patch| {
                FacePatch {
                    face: patch
                        .face
                        .map(|face| FaceIndex::new(first_face + usize::from(face))),
                    ..patch
                }
            }));
        }
        Ok(patches)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::{
        apertures::CircleShape,
        degree,
        geometry::{Face, FaceId, Geometry, Interface, ParametricSolid, Part, Side, SurfaceShape},
        material::Material,
        millimeter,
        refractive_index::RefrIndexConst,
        utils::geom_transformation::Isometry,
    };
    use uom::si::{f64::Length, length::millimeter};

    fn circle() -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(12.5))?, None)
    }
    /// Glass with a refractive index of 1.5, the material of every part below.
    fn glass() -> OpmResult<Material> {
        Ok(Material::from(&RefrIndexConst::new(1.5)?))
    }
    /// The crown lens of a doublet at the origin, its rear face (R = -40 mm) 5 mm behind it.
    fn crown_part() -> OpmResult<Part> {
        Part::new(
            &Isometry::identity(),
            Geometry::singlet(
                millimeter!(60.0),
                millimeter!(-40.0),
                millimeter!(5.0),
                circle()?,
                glass()?,
            )?,
        )
    }
    /// The flint lens of a doublet with the given front radius, at the given placement.
    fn flint_part(front_radius: Length, placement: &Isometry) -> OpmResult<Part> {
        Part::new(
            placement,
            Geometry::singlet(
                front_radius,
                millimeter!(f64::INFINITY),
                millimeter!(5.0),
                circle()?,
                glass()?,
            )?,
        )
    }
    /// The crown lens at the origin and a second lens 30 mm aside and 8 mm further along z, without
    /// an interface between them.
    fn side_by_side() -> OpmResult<Assembly> {
        let aside = Isometry::new_translation(millimeter!(30.0, 0.0, 8.0))?;
        Assembly::new(
            vec![crown_part()?, flint_part(millimeter!(-40.0), &aside)?],
            vec![],
        )
    }
    /// The diagonal of a beam splitter cube: tilted by 45° about x, 10 mm along z.
    fn diagonal() -> OpmResult<Face> {
        Ok(Face::new(
            SurfaceShape::Plane,
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(45.0, 0.0, 0.0))?,
        ))
    }
    /// A prism of a beam splitter cube between the given faces.
    fn prism_part(front: (Face, Side), rear: (Face, Side)) -> OpmResult<Part> {
        Part::new(
            &Isometry::identity(),
            Geometry::Parametric(Box::new(ParametricSolid::new(vec![front, rear], glass()?)?)),
        )
    }
    /// A beam splitter cube: two prisms cemented along the diagonal. A parametric solid cannot be
    /// drawn yet.
    fn cube() -> OpmResult<Assembly> {
        let entrance = Face::new(SurfaceShape::Plane, Isometry::identity());
        let exit = Face::new(
            SurfaceShape::Plane,
            Isometry::new_along_z(millimeter!(20.0))?,
        );
        Assembly::new(
            vec![
                prism_part((entrance, Side::Behind), (diagonal()?, Side::InFront))?,
                prism_part((diagonal()?, Side::Behind), (exit, Side::InFront))?,
            ],
            vec![Interface::new(
                FaceId::new(0, FaceIndex::new(1)),
                FaceId::new(1, FaceIndex::new(0)),
            )],
        )
    }
    /// An assembly is drawn as the patches of its parts one after the other, each where the
    /// assembly places it and on the face the assembly numbers it as; a part that cannot be drawn
    /// leaves the whole assembly undrawn.
    #[test]
    fn an_assembly_is_drawn_as_its_parts() -> OpmResult<()> {
        let unbounded = circle()?;
        let meshes = Meshable::mesh(&side_by_side()?, 16, &unbounded)?;
        assert_eq!(meshes.len(), 6, "each lens is drawn as three patches");
        let faces: Vec<_> = meshes.iter().map(|patch| patch.face).collect();
        let face = |index| Some(FaceIndex::new(index));
        assert_eq!(faces, [face(0), face(1), None, face(2), face(3), None]);
        // The second lens sits 30 mm aside, the first one around the axis of the assembly.
        for (patches, center) in [(&meshes[..3], 0.0), (&meshes[3..], 30.0)] {
            for point in patches.iter().flat_map(|patch| patch.mesh.points()) {
                let off_center =
                    (point.x.get::<millimeter>() - center).hypot(point.y.get::<millimeter>());
                assert!(
                    off_center <= 12.5 + 1e-9,
                    "a point lies {off_center} mm off its lens"
                );
            }
        }
        let error = Meshable::mesh(&cube()?, 16, &unbounded).unwrap_err();
        assert!(error.to_string().contains("cannot be meshed"), "{error}");
        Ok(())
    }
}
