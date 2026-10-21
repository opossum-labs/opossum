//! The runtime state of a placed component: its geometry, the optic surfaces traced at its faces and
//! the inversion of the medium it encloses.

use std::collections::BTreeMap;

use crate::{
    core_optics::optic_surface::OpticSurface,
    error::{OpmResult, OpossumError},
    gain::Inversion,
    geometry::{Geometry, body::Body},
};

/// The optic surfaces of a node, one per port name.
#[derive(Default, Debug, Clone)]
pub struct RuntimeSurfaces {
    /// Mapping of input port names to optical surfaces.
    pub inputs: BTreeMap<String, OpticSurface>,
    /// Mapping of output port names to optical surfaces.
    pub outputs: BTreeMap<String, OpticSurface>,
}

impl RuntimeSurfaces {
    /// Returns an iterator over all mutable optic surfaces (inputs and outputs).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.inputs.values_mut().chain(self.outputs.values_mut())
    }

    /// Returns an iterator over all optic surfaces (inputs and outputs) and their port names.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &OpticSurface)> {
        self.inputs.iter().chain(self.outputs.iter())
    }

    /// Returns the optic surface under the given port name, input or output.
    #[must_use]
    pub fn get(&self, surf_name: &str) -> Option<&OpticSurface> {
        self.inputs
            .get(surf_name)
            .or_else(|| self.outputs.get(surf_name))
    }

    /// Returns the optic surface under the given port name, input or output, mutably.
    pub fn get_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        self.inputs
            .get_mut(surf_name)
            .or_else(|| self.outputs.get_mut(surf_name))
    }
}

/// The geometry of a node as an analysis uses it: moved to where the node sits, together with the
/// optic surface of every port, each a face of it, and the inversion of the medium it encloses.
///
/// It is built by [`OpticNodeExt::install_geometry`](crate::core_optics::OpticNodeExt::install_geometry)
/// and never written to a file.
#[derive(Debug, Clone)]
pub struct RuntimeGeometry {
    geometry: Geometry,
    surfaces: RuntimeSurfaces,
    inversion: Option<Inversion>,
}

impl RuntimeGeometry {
    /// Creates the runtime state of a placed geometry with its optic surfaces and an empty
    /// inversion.
    ///
    /// # Arguments
    ///
    /// * `geometry` - the geometry, moved to where the node sits.
    /// * `surfaces` - the optic surfaces, each a face of `geometry`.
    ///
    /// # Errors
    ///
    /// This function returns an error if a surface sits on a face the geometry does not have.
    pub fn new(geometry: Geometry, surfaces: RuntimeSurfaces) -> OpmResult<Self> {
        let kind = geometry.kind();
        if let Some((name, surface)) = surfaces
            .iter()
            .find(|(_, surface)| kind.face(surface.face()).is_none())
        {
            return Err(OpossumError::Other(format!(
                "surface '{name}' sits on face {}, which does not exist (face count: {})",
                surface.face(),
                kind.face_count()
            )));
        }
        Ok(Self {
            geometry,
            surfaces,
            inversion: None,
        })
    }
    /// Returns the placed geometry.
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }
    /// Returns the optic surfaces.
    #[must_use]
    pub const fn surfaces(&self) -> &RuntimeSurfaces {
        &self.surfaces
    }
    /// Returns the optic surface under the given port name, input or output, mutably.
    pub fn surface_mut(&mut self, surf_name: &str) -> Option<&mut OpticSurface> {
        self.surfaces.get_mut(surf_name)
    }
    /// Returns an iterator over all optic surfaces, mutably.
    pub fn surfaces_mut(&mut self) -> impl Iterator<Item = &mut OpticSurface> {
        self.surfaces.values_mut()
    }
    /// Returns an optic surface together with the geometry it is a face of, which answers where a
    /// ray hits it.
    ///
    /// # Arguments
    ///
    /// * `surf_name` - the name of the surface (its port name).
    ///
    /// # Returns
    ///
    /// The geometry and the surface, or `None` if there is no such surface.
    pub fn traced_surface_mut(
        &mut self,
        surf_name: &str,
    ) -> Option<(&Geometry, &mut OpticSurface)> {
        Some((&self.geometry, self.surfaces.get_mut(surf_name)?))
    }
    /// Returns the medium: the body the geometry encloses and the inversion in it.
    ///
    /// [`OpticNode::prepare_volume`](crate::core_optics::OpticNode::prepare_volume) fills the
    /// inversion for an analysis run, and
    /// [`OpticNode::reset_data`](crate::core_optics::OpticNode::reset_data) clears it again.
    ///
    /// # Returns
    ///
    /// The body and the inversion, if a model built one; or `None` for a single surface, which
    /// encloses no volume.
    #[must_use]
    pub fn medium(&self) -> Option<(&dyn Body, Option<&Inversion>)> {
        Some((self.geometry.kind().as_body()?, self.inversion.as_ref()))
    }
    /// Returns the medium as [`RuntimeGeometry::medium`] does, with the inversion to be built or
    /// depleted.
    pub fn medium_mut(&mut self) -> Option<(&dyn Body, &mut Option<Inversion>)> {
        Some((self.geometry.kind().as_body()?, &mut self.inversion))
    }
    /// Clears the inversion of the medium between analysis runs.
    pub fn clear_inversion(&mut self) {
        self.inversion = None;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        J_per_cm2,
        apertures::{CircleShape, Outline},
        coatings::CoatingType,
        geometry::Extruded,
        millimeter,
    };

    /// A disk 10 mm thick and 5 mm in radius.
    fn test_body() -> OpmResult<Geometry> {
        Geometry::singlet(
            millimeter!(f64::INFINITY),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?,
        )
    }
    /// The surfaces of a disk: `input_1` on the front face and `output_1` on the given face.
    fn disk_surfaces(output_face: usize) -> OpmResult<RuntimeSurfaces> {
        let surface = |face| OpticSurface::new(face, CoatingType::IdealAR, J_per_cm2!(1.0));
        let mut surfaces = RuntimeSurfaces::default();
        surfaces
            .inputs
            .insert("input_1".into(), surface(Extruded::FRONT)?);
        surfaces
            .outputs
            .insert("output_1".into(), surface(output_face)?);
        Ok(surfaces)
    }
    #[test]
    fn a_single_surface_has_no_medium() -> OpmResult<()> {
        assert!(
            RuntimeGeometry::new(test_body()?, RuntimeSurfaces::default())?
                .medium()
                .is_some()
        );
        assert!(
            RuntimeGeometry::new(Geometry::plane(None), RuntimeSurfaces::default())?
                .medium()
                .is_none()
        );
        Ok(())
    }
    /// Every surface is a face of the geometry: a disk has two faces, so there is no face 2.
    #[test]
    fn a_surface_on_a_missing_face_is_rejected() -> OpmResult<()> {
        assert!(RuntimeGeometry::new(test_body()?, disk_surfaces(Extruded::REAR)?).is_ok());
        let error = RuntimeGeometry::new(test_body()?, disk_surfaces(2)?)
            .expect_err("a surface on face 2 of a disk was accepted");
        assert!(
            error
                .to_string()
                .contains("surface 'output_1' sits on face 2"),
            "{error}"
        );
        Ok(())
    }
    /// The medium starts without an inversion; building one fills it, and clearing empties it.
    #[test]
    fn set_and_clear_the_inversion() -> OpmResult<()> {
        let mut runtime = RuntimeGeometry::new(test_body()?, RuntimeSurfaces::default())?;
        let has_inversion = |runtime: &RuntimeGeometry| {
            runtime
                .medium()
                .is_some_and(|(_, inversion)| inversion.is_some())
        };
        assert!(!has_inversion(&runtime));
        let (_, inversion) = runtime
            .medium_mut()
            .ok_or_else(|| OpossumError::Other("the disk has no medium".into()))?;
        *inversion = Some(Inversion::Uniform(1.0));
        assert!(has_inversion(&runtime));
        runtime.clear_inversion();
        assert!(!has_inversion(&runtime));
        Ok(())
    }
}
