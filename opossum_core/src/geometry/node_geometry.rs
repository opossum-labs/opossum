#![warn(missing_docs)]
//! The shape of an optical component: its faces and where they end, in frames.
//!
//! A node's properties (curvatures, thickness, optic shape, ...) are what a user enters; the node
//! derives a [`Geometry`] from them (see
//! [`OpticNode::create_geometry`](crate::core_optics::OpticNode::create_geometry)), stated
//! relative to the node.
//! Moved to where the node sits (see [`Geometry::move_by`]), the same geometry is what rays are
//! traced against. The node type thereby acts as a preset: a lens is a [`Geometry::singlet`] whose
//! parameters are the lens's properties.
//!
//! A component is a single surface ([`Geometry::Surface`]) or one of the kinds of solid enclosing a
//! volume of material. Every kind is described in a file of its own and answers what
//! [`GeometryKind`] asks; [`Geometry::kind`] is the one place that tells the kinds apart.

use nalgebra::{Point3, Vector3};
use num_traits::Zero;
use uom::si::f64::{Angle, Length};

use log::warn;
use uuid::Uuid;

use crate::{
    analyzers::propagation_strategy::{MissedSurfaceStrategy, PropagationStrategy},
    apertures::Outline,
    core_optics::{optic_surface::OpticSurface, surface_effect::SurfaceEffect},
    error::{OpmResult, OpossumError},
    geometry::{
        Assembly, Extruded, Face, FaceIndex, ParametricSolid, Rim, SurfaceShape, body::Body,
    },
    light::{Ray, Rays, rays::ReflectedPart},
    utils::geom_transformation::Isometry,
};

/// The shape of an optical component, with the optic surface of every face.
#[derive(Debug, Clone, PartialEq)]
pub enum Geometry {
    /// A single surface without thickness, such as a thin mirror, a grating or a detector.
    Surface(Box<SurfaceGeometry>),
    /// Two faces capping a profile extruded along an axis: lens, cylindric lens, wedge, plate.
    Extruded(Box<Extruded>),
    /// A solid bounded by several faces: the intersection of the inner sides of all of them.
    Parametric(ParametricSolid),
    /// Several solids, each with its own material, sharing interfaces.
    Assembly(Assembly),
}

impl Geometry {
    /// A spherical singlet lens: two spherical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the outline.
    pub fn singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        outline: Outline,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::spherical(front_radius)?,
            SurfaceShape::spherical(rear_radius)?,
            Isometry::new_along_z(center_thickness)?,
            outline,
        )
    }
    /// A cylindrical singlet lens: two cylindrical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the outline.
    pub fn cylindrical_singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        outline: Outline,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::cylindrical(front_radius)?,
            SurfaceShape::cylindrical(rear_radius)?,
            Isometry::new_along_z(center_thickness)?,
            outline,
        )
    }
    /// A wedge: two flat faces, the rear one tilted about the local x axis.
    ///
    /// # Arguments
    ///
    /// * `center_thickness` - distance between the two faces on the axis.
    /// * `wedge_angle` - tilt of the rear face.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness or the angle is not finite.
    pub fn wedge(
        center_thickness: Length,
        wedge_angle: Angle,
        outline: Outline,
    ) -> OpmResult<Self> {
        let tilt = Isometry::new(
            Point3::origin(),
            Point3::new(wedge_angle, Angle::zero(), Angle::zero()),
        )?;
        Self::two_faced(
            SurfaceShape::Plane,
            SurfaceShape::Plane,
            Isometry::new_along_z(center_thickness)?.append(&tilt),
            outline,
        )
    }
    /// A single surface.
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the outline relative to the node; its z axis is the direction the
    ///   outline is projected along. The identity for a surface used along its own axis.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `outline` - its transversal extent in the xy plane of the `axis` frame, or `None` for an
    ///   unbounded (virtual) surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// outline.
    pub fn surface(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        outline: Option<Outline>,
    ) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            axis,
            shape,
            vertex,
            outline,
            SurfaceExtent::Edge,
        )?)))
    }
    /// A detector: a surface at the node that records the light within its outline and lets all
    /// of it pass (see [`SurfaceExtent::Window`]).
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface the light is recorded on.
    /// * `outline` - the window the light is recorded within, or `None` to record all of it.
    ///
    /// # Errors
    ///
    /// See [`Geometry::surface`].
    pub fn detector(shape: SurfaceShape, outline: Option<Outline>) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            Isometry::identity(),
            shape,
            Isometry::identity(),
            outline,
            SurfaceExtent::Window,
        )?)))
    }
    /// A flat surface at the node itself.
    ///
    /// # Arguments
    ///
    /// * `outline` - its transversal extent, or `None` for an unbounded (virtual) surface.
    #[must_use]
    pub fn plane(outline: Option<Outline>) -> Self {
        // A plane reaches every outline, so nothing is left to check.
        Self::Surface(Box::new(SurfaceGeometry {
            surface: Face::new(SurfaceShape::Plane, Isometry::identity()).into(),
            extent: outline.map_or(SurfaceExtent::Open, |outline| {
                SurfaceExtent::Edge(Rim::new(outline, Isometry::identity()))
            }),
        }))
    }
    /// An extruded solid along the node's own axis whose front vertex sits at the node.
    ///
    /// # Arguments
    ///
    /// * `front` - the shape of the front face.
    /// * `rear` - the shape of the rear face.
    /// * `rear_vertex` - position and orientation of the rear face's vertex relative to the node.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// See [`Extruded::new`].
    fn two_faced(
        front: SurfaceShape,
        rear: SurfaceShape,
        rear_vertex: Isometry,
        outline: Outline,
    ) -> OpmResult<Self> {
        Ok(Self::Extruded(Box::new(Extruded::new(
            Isometry::identity(),
            Face::new(front, Isometry::identity()),
            Face::new(rear, rear_vertex),
            outline,
        )?)))
    }
    /// Move this geometry by an isometry, such as the placement of its node.
    ///
    /// Every face and every rim is moved; the geometry is the same thing at another place.
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this geometry is stated in, given in the frame it is moved to.
    pub fn move_by(&mut self, by: &Isometry) {
        self.kind_mut().move_by(by);
    }
    /// Return the kind of geometry this is, as the one thing every question is asked of.
    #[must_use]
    pub fn kind(&self) -> &dyn GeometryKind {
        match self {
            Self::Surface(surface) => &**surface,
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
    /// Return the kind of geometry this is, as [`Geometry::kind`] does, mutably.
    pub fn kind_mut(&mut self) -> &mut dyn GeometryKind {
        match self {
            Self::Surface(surface) => &mut **surface,
            Self::Extruded(extruded) => &mut **extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
    /// Return the face with the given number (see [`GeometryKind::surface`]).
    #[must_use]
    pub fn face(&self, face: FaceIndex) -> Option<&Face> {
        self.kind().surface(face).map(OpticSurface::face)
    }
    /// Return every optic surface of this geometry with the number of its face, in the order the
    /// faces are numbered.
    pub fn surfaces(&self) -> impl Iterator<Item = (FaceIndex, &OpticSurface)> {
        let kind = self.kind();
        (0..kind.face_count())
            .map(FaceIndex::new)
            .filter_map(move |face| Some((face, kind.surface(face)?)))
    }
    /// Return where a ray meets one face of this geometry.
    ///
    /// The ray meets the unbounded surface of the face (see [`Face::intersect`]), and the point
    /// counts only where the face reaches (see [`GeometryKind::includes`]).
    ///
    /// # Arguments
    ///
    /// * `face` - the number of the face.
    /// * `ray` - the ray, in the frame this geometry is stated in.
    ///
    /// # Returns
    ///
    /// The hit; or `None` if there is no such face, if the ray misses its surface or if it meets
    /// the surface beyond the edge of the face.
    #[must_use]
    pub fn intersect(&self, face: FaceIndex, ray: &Ray) -> Option<SurfaceHit> {
        let kind = self.kind();
        let surface = kind.surface(face)?.face();
        let (point, normal) = surface.intersect(ray)?;
        kind.includes(face, &point).then(|| SurfaceHit {
            point,
            normal,
            local_point: surface.frame().inverse_transform_point(&point),
            recorded: kind.window().is_none_or(|window| window.contains(&point)),
        })
    }
}

impl Geometry {
    /// Return the optic surface of one face, mutably.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such face.
    fn surface_of_mut(&mut self, face: FaceIndex) -> OpmResult<&mut OpticSurface> {
        self.kind_mut()
            .surface_mut(face)
            .ok_or_else(|| OpossumError::Analysis(format!("there is no face {face}")))
    }
    /// Let the surface of one face of this component act on a bundle of rays.
    ///
    /// The geometry answers where each valid ray hits the face (see [`Geometry::intersect`]). A ray
    /// that misses it follows the missed surface strategy. At a hit, the effect acts on the ray (see
    /// [`SurfaceEffect::act_at`]) and, if the ray sends a part back, on its helper rays as well; the
    /// surface records the hit in its hit map. Which of the two parts is the intended beam is
    /// recorded on both (see [`ReflectedPart`]).
    ///
    /// # Arguments
    ///
    /// * `face` - the number of the face.
    /// * `rays` - the bundle. Its rays are moved to the surface and continue as the transmitted part.
    /// * `effect` - what the surface does to a ray that hits it.
    /// * `part` - what the reflected part is to the bundle.
    /// * `missed` - what happens to a ray that misses the surface.
    ///
    /// # Returns
    ///
    /// The bundle of reflected parts.
    ///
    /// # Warnings
    ///
    /// This function warns if a ray missed the surface or sent nothing back, and if the bundle has
    /// no valid ray.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such face, if the effect fails for a ray or if
    /// a hit cannot be recorded.
    pub fn interact_with_rays(
        &mut self,
        face: FaceIndex,
        rays: &mut Rays,
        effect: &dyn SurfaceEffect,
        part: ReflectedPart,
        missed: MissedSurfaceStrategy,
    ) -> OpmResult<Rays> {
        let coating = *self.surface_of_mut(face)?.coating();
        let uuid = rays.uuid();
        let mut valid_rays_found = false;
        let mut rays_missed = false;
        let mut reflected_rays = Rays::with_capacity(rays.nr_of_rays(false));
        // The hits are recorded once the geometry has answered for every ray.
        let mut hit_points = Vec::new();
        for ray in rays.iter_mut().filter(|ray| ray.valid()) {
            valid_rays_found = true;
            let Some(surface_hit) = self.intersect(face, ray) else {
                missed.apply_to(ray);
                rays_missed = true;
                continue;
            };
            let (reflected, hit_point) = effect.act_at(ray, &surface_hit, &coating)?;
            if let Some(mut reflected) = reflected {
                if let (Some(helpers), Some(reflected_helpers)) =
                    (ray.helper_rays_mut(), reflected.helper_rays_mut())
                {
                    for (helper, reflected_helper) in
                        helpers.iter_mut().zip(reflected_helpers.iter_mut())
                    {
                        let Some(helper_hit) = self.intersect(face, helper) else {
                            missed.apply_to(helper);
                            continue;
                        };
                        if let (Some(helper_reflected), _) =
                            effect.act_at(helper, &helper_hit, &coating)?
                        {
                            reflected_helper.set_direction(helper_reflected.direction())?;
                        }
                    }
                }
                part.record_split(ray, &mut reflected);
                reflected_rays.add_ray(reflected);
            } else {
                rays_missed = true;
            }
            if let Some(hit_point) = hit_point {
                hit_points.push((hit_point, ray.number_of_bounces()));
            }
        }
        let surface = self.surface_of_mut(face)?;
        for (hit_point, bounces) in hit_points {
            surface.add_to_hit_map(hit_point, bounces, uuid)?;
        }
        if rays_missed {
            warn!("rays totally reflected or missed a surface");
        }
        if !valid_rays_found {
            warn!("ray bundle contains no valid rays - not propagating");
        }
        part.link_bundles(rays, &mut reflected_rays);
        Ok(reflected_rays)
    }
    /// Let the surface of one face of this component act on the bundles of an analysis (see
    /// [`Geometry::interact`]).
    ///
    /// After each bundle, the analysis does what it does at a surface (see
    /// [`PropagationStrategy::on_surface_interaction`]). Then the ghost reflections cached at the
    /// surface for this direction are appended to the bundles.
    ///
    /// # Arguments
    ///
    /// * `face` - the number of the face.
    /// * `bundles` - the bundles of the analysis.
    /// * `effect` - what the surface does to a ray that hits it.
    /// * `part` - what the reflected part is to each bundle.
    /// * `backward` - whether the light runs through the node backward.
    /// * `node` - the node of this component, which the reflected bundles originate from.
    /// * `strategy` - the analysis.
    ///
    /// # Errors
    ///
    /// This function returns an error if there is no such face, if the interaction fails or if the
    /// analysis fails at the surface (e.g. in its fluence evaluation).
    #[allow(clippy::too_many_arguments)]
    pub fn propagate_rays(
        &mut self,
        face: FaceIndex,
        bundles: &mut Vec<Rays>,
        effect: &dyn SurfaceEffect,
        part: ReflectedPart,
        backward: bool,
        node: Uuid,
        strategy: &dyn PropagationStrategy,
    ) -> OpmResult<()> {
        let missed = strategy.missed_surface_strategy();
        for rays in &mut *bundles {
            let mut reflected = self.interact_with_rays(face, rays, effect, part, missed)?;
            reflected.set_node_origin_uuid(node);
            strategy.on_surface_interaction(self.surface_of_mut(face)?, rays, reflected, backward)?;
        }
        bundles.extend_from_slice(self.surface_of_mut(face)?.get_rays_cache(backward));
        Ok(())
    }
}

/// An unbounded surface made of one face, such as a virtual surface a ray is traced to.
impl From<Face> for Geometry {
    fn from(face: Face) -> Self {
        Self::Surface(Box::new(SurfaceGeometry {
            surface: face.into(),
            extent: SurfaceExtent::Open,
        }))
    }
}

/// Where a ray meets a face of a component (see [`Geometry::intersect`]).
///
/// It holds all that the ray needs to know about the component to act on the hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceHit {
    point: Point3<Length>,
    normal: Vector3<f64>,
    local_point: Point3<Length>,
    recorded: bool,
}

impl SurfaceHit {
    /// Return where the ray meets the face, in the frame the geometry is stated in.
    #[must_use]
    pub const fn point(&self) -> Point3<Length> {
        self.point
    }
    /// Return the normal of the face at the hit, normalized and pointing against the ray.
    #[must_use]
    pub const fn normal(&self) -> Vector3<f64> {
        self.normal
    }
    /// Return where the ray meets the face, in the frame of the face: what a hit map records and
    /// where a grating measures its phase from.
    #[must_use]
    pub const fn local_point(&self) -> Point3<Length> {
        self.local_point
    }
    /// Return whether the hit is recorded: it is not only outside the window of a detector (see
    /// [`GeometryKind::window`]).
    #[must_use]
    pub const fn recorded(&self) -> bool {
        self.recorded
    }
}

/// What every kind of geometry answers about its faces, its volume and its placement.
///
/// The faces of a geometry are numbered. What a face means, for instance which one light enters
/// through, is not decided here but by the node using the geometry. Every kind answers each
/// question itself.
pub trait GeometryKind {
    /// Return the optic surface of the face with the given number.
    ///
    /// # Returns
    ///
    /// The surface, its face in the frame the geometry is stated in, or `None` if there is no such
    /// face.
    fn surface(&self, face: FaceIndex) -> Option<&OpticSurface>;
    /// Return the optic surface of the face with the given number, mutably: for its coating and
    /// what an analysis records there.
    fn surface_mut(&mut self, face: FaceIndex) -> Option<&mut OpticSurface>;
    /// Return the number of faces.
    fn face_count(&self) -> usize;
    /// Return the name of the face with the given number, such as "front surface".
    ///
    /// A node names the property holding the coating and the LIDT of a face after it (see
    /// [`SurfaceConfig`](crate::core_optics::SurfaceConfig)).
    ///
    /// # Returns
    ///
    /// The name, or `None` if there is no such face.
    fn face_name(&self, face: FaceIndex) -> Option<String>;
    /// Return whether a point on the unbounded surface of a face belongs to the component.
    ///
    /// The surface of a face is unbounded: a plane reaches in every direction, a sphere closes on
    /// itself. Which of its points belong to the component is the lateral boundary of the face, and
    /// it depends on the kind: an extruded solid ends at the [`Rim`] of its profile, the faces of a
    /// parametric solid end at each other, a part of an assembly ends where its own solid does. A
    /// ray meets a face where its surface does and this agrees.
    ///
    /// # Arguments
    ///
    /// - `face`: the number of the face the point lies on.
    /// - `point`: a point on the surface of that face, in the frame the geometry is stated in.
    ///
    /// # Returns
    ///
    /// `true` if the face reaches to this point; `false` if the point lies on the surface beyond
    /// the edge of the face, or if there is no such face.
    fn includes(&self, face: FaceIndex, point: &Point3<Length>) -> bool;
    /// Return the window a detector records within, or `None` if all light on its faces is recorded.
    ///
    /// A window is no boundary: light outside it passes the face like light inside, it is just not
    /// recorded (see [`GeometryKind::includes`] for where a face ends).
    fn window(&self) -> Option<&Rim>;
    /// Return the volume this geometry encloses, or `None` for a single surface, which encloses
    /// none.
    fn as_body(&self) -> Option<&dyn Body>;
    /// Return the volume this geometry encloses, as [`GeometryKind::as_body`] does, mutably: for
    /// the inversion a gain model builds or depletes in it.
    fn as_body_mut(&mut self) -> Option<&mut dyn Body>;
    /// Move this geometry by an isometry (see [`Geometry::move_by`]).
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this geometry is stated in, given in the frame it is moved to.
    fn move_by(&mut self, by: &Isometry);
}

/// A single surface without thickness.
///
/// The face is stated in the frame of the geometry. The rim of its extent, if there is one, keeps
/// the axis its outline is extruded along; an off-axis parabola uses it to measure its optic shape
/// parallel to the axis of its parent parabola. An open surface has no rim and so no axis of its
/// own, which is why the axis is already applied to the face.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceGeometry {
    surface: OpticSurface,
    extent: SurfaceExtent,
}

/// How far a [`SurfaceGeometry`] reaches sideways, and what its rim means for a ray outside it.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceExtent {
    /// The surface is unbounded.
    Open,
    /// The component ends at the rim: a ray outside misses it, and the analyzer's missed surface
    /// strategy decides what happens to the ray.
    Edge(Rim),
    /// The rim only bounds what a detector records: a ray outside passes the surface like one
    /// inside, but is not recorded and leaves no hit.
    Window(Rim),
}

impl SurfaceExtent {
    /// Move the rim of this extent, if there is one, by an isometry (see [`Rim::move_by`]).
    fn move_by(&mut self, by: &Isometry) {
        match self {
            Self::Open => {}
            Self::Edge(rim) | Self::Window(rim) => rim.move_by(by),
        }
    }
}

impl SurfaceGeometry {
    /// Create a new [`SurfaceGeometry`].
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the outline relative to the node.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `outline` - its transversal extent, or `None` for an unbounded surface.
    /// * `extent` - what the rim of the outline means for a ray outside it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// outline.
    fn new(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        outline: Option<Outline>,
        extent: fn(Rim) -> SurfaceExtent,
    ) -> OpmResult<Self> {
        let face = Face::new(shape, axis.append(&vertex));
        if let Some(outline) = &outline {
            face.check_reach(outline.transversal_reach(), "surface")?;
        }
        Ok(Self {
            surface: face.into(),
            extent: outline.map_or(SurfaceExtent::Open, |outline| {
                extent(Rim::new(outline, axis))
            }),
        })
    }
    /// The number of the one face of a surface.
    pub const FACE: FaceIndex = FaceIndex::new(0);
}

/// A surface ends at its edge, if it has one. The window of a detector is no edge: the surface
/// reaches as far as light does.
impl GeometryKind for SurfaceGeometry {
    fn surface(&self, face: FaceIndex) -> Option<&OpticSurface> {
        (face == Self::FACE).then_some(&self.surface)
    }
    fn surface_mut(&mut self, face: FaceIndex) -> Option<&mut OpticSurface> {
        (face == Self::FACE).then_some(&mut self.surface)
    }
    fn face_count(&self) -> usize {
        1
    }
    fn face_name(&self, face: FaceIndex) -> Option<String> {
        (face == Self::FACE).then(|| "surface".into())
    }
    fn includes(&self, face: FaceIndex, point: &Point3<Length>) -> bool {
        GeometryKind::surface(self, face).is_some_and(|surface| match &self.extent {
            SurfaceExtent::Edge(rim) => rim.bounds(surface.face(), point),
            SurfaceExtent::Open | SurfaceExtent::Window(_) => true,
        })
    }
    fn window(&self) -> Option<&Rim> {
        match &self.extent {
            SurfaceExtent::Window(rim) => Some(rim),
            SurfaceExtent::Open | SurfaceExtent::Edge(_) => None,
        }
    }
    fn as_body(&self) -> Option<&dyn Body> {
        None
    }
    fn as_body_mut(&mut self) -> Option<&mut dyn Body> {
        None
    }
    fn move_by(&mut self, by: &Isometry) {
        self.surface.move_by(by);
        self.extent.move_by(by);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        analyzers::RayTraceConfig,
        apertures::CircleShape,
        coatings::{CoatingConstantR, CoatingType},
        core_optics::surface_effect::Refraction,
        degree,
        distributions::{fluence::general_gaussian::General2DGaussian, position::Hexapolar},
        gain::Inversion,
        geometry::{Parabola, Part, Side, Sphere},
        joule,
        light::Spectrum,
        material::Material,
        meter, millimeter, nanometer,
        nodes::{
            SplittingConfig,
            ideal_filter::{EdgeFilter, EdgeFilterType},
        },
        percent,
        properties::proptype::AssetRef,
        radian,
        refractive_index::{RefrIndexConst, refr_index_vaccuum},
        utils::test_helper::helper::check_logs,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::vector;
    use uom::si::{energy::joule, f64::Energy, length::millimeter};

    fn circle() -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(12.5))?, Vec::new())
    }
    /// Return the frame of the face with the given number.
    fn frame_of(geometry: &Geometry, index: FaceIndex) -> OpmResult<Isometry> {
        geometry
            .face(index)
            .map(|face| *face.frame())
            .ok_or_else(|| OpossumError::Other(format!("there is no face {index}")))
    }
    /// A copy of the geometry, moved by the given isometry.
    fn moved_copy(geometry: &Geometry, by: &Isometry) -> Geometry {
        let mut moved = geometry.clone();
        moved.move_by(by);
        moved
    }
    #[test]
    fn a_surface_has_one_face() {
        let plane = Geometry::plane(None);
        assert_eq!(plane.kind().face_count(), 1);
        assert!(plane.face(SurfaceGeometry::FACE).is_some());
        assert!(plane.face(FaceIndex::new(1)).is_none());
    }
    #[test]
    fn a_singlet_has_its_rear_vertex_one_center_thickness_behind_the_front() -> OpmResult<()> {
        let geometry = Geometry::singlet(
            millimeter!(50.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            circle()?,
        )?;
        assert_eq!(geometry.kind().face_count(), 2);
        assert_abs_diff_eq!(
            frame_of(&geometry, Extruded::FRONT)?
                .translation()
                .z
                .get::<millimeter>(),
            0.0
        );
        assert_abs_diff_eq!(
            frame_of(&geometry, Extruded::REAR)?
                .translation()
                .z
                .get::<millimeter>(),
            5.0
        );
        Ok(())
    }
    #[test]
    fn a_wedge_tilts_its_rear_face() -> OpmResult<()> {
        let geometry = Geometry::wedge(millimeter!(10.0), degree!(5.0), circle()?)?;
        let normal =
            frame_of(&geometry, Extruded::REAR)?.transform_vector_f64(&nalgebra::Vector3::z());
        assert_abs_diff_eq!(normal.y, -(5.0_f64.to_radians().sin()), epsilon = 1e-12);
        Ok(())
    }
    /// A geometry moved to a node has every face at the node followed by where the face sits in the
    /// geometry, and every rim along with it.
    #[test]
    fn a_moved_geometry_has_every_face_and_rim_at_its_node() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometries = [
            Geometry::singlet(
                millimeter!(50.0),
                millimeter!(-80.0),
                millimeter!(5.0),
                circle()?,
            )?,
            Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?,
        ];
        for geometry in geometries {
            let moved = moved_copy(&geometry, &node_frame);
            assert_eq!(moved.kind().face_count(), geometry.kind().face_count());
            for index in (0..geometry.kind().face_count()).map(FaceIndex::new) {
                assert_eq!(
                    frame_of(&moved, index)?,
                    node_frame.append(&frame_of(&geometry, index)?)
                );
            }
        }
        // The rim moves along: a point on the moved face 12 mm off its axis is still on it.
        let surface = Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?;
        let point = millimeter!(23.0, 0.0, 12.0);
        assert!(surface.kind().includes(SurfaceGeometry::FACE, &point));
        assert!(
            moved_copy(&surface, &node_frame)
                .kind()
                .includes(SurfaceGeometry::FACE, &node_frame.transform_point(&point))
        );
        Ok(())
    }
    /// Every kind of solid has its faces numbered, encloses a volume that holds an inversion and
    /// moves its faces along; a single surface encloses none.
    #[test]
    fn every_kind_of_solid_numbers_its_faces_and_encloses_a_volume() -> OpmResult<()> {
        let parametric = Geometry::Parametric(ParametricSolid::new(vec![(
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Side::Behind,
        )])?);
        let glass = AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?));
        let part = Part::new(&Isometry::identity(), parametric.clone(), glass)?;
        let assembly = Geometry::Assembly(Assembly::new(vec![part], vec![])?);
        let frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        for mut geometry in [parametric, assembly] {
            assert_eq!(geometry.kind().face_count(), 1);
            assert_eq!(
                geometry.kind().face_name(FaceIndex::new(0)).as_deref(),
                Some("surface 1")
            );
            assert!(geometry.kind().face_name(FaceIndex::new(1)).is_none());
            geometry
                .surface_of_mut(FaceIndex::new(0))?
                .set_coating(CoatingType::Fresnel);
            assert!(matches!(
                geometry
                    .kind()
                    .surface(FaceIndex::new(0))
                    .map(OpticSurface::coating),
                Some(CoatingType::Fresnel)
            ));
            assert!(geometry.kind().as_body().is_some());
            assert_eq!(
                frame_of(&moved_copy(&geometry, &frame), FaceIndex::new(0))?,
                frame.append(&frame_of(&geometry, FaceIndex::new(0))?)
            );
            let body = geometry
                .kind_mut()
                .as_body_mut()
                .ok_or_else(|| OpossumError::Other("the solid encloses no volume".into()))?;
            *body.inversion_mut() = Some(Inversion::Uniform(1.0));
            assert_eq!(
                geometry.kind().as_body().and_then(Body::inversion),
                Some(&Inversion::Uniform(1.0))
            );
        }
        assert!(Geometry::plane(None).kind_mut().as_body_mut().is_none());
        Ok(())
    }
    /// A curved face has to reach as far out as the outline it is bounded by; a sphere or a
    /// cylinder ends at its radius of curvature. Exactly reaching it (a hemisphere) is still a shape.
    #[test]
    fn an_outline_beyond_the_curvature_is_rejected() -> OpmResult<()> {
        let flat = millimeter!(f64::INFINITY);
        for radius in [10.0, -10.0] {
            let curved = millimeter!(radius);
            for (front, rear) in [(curved, flat), (flat, curved)] {
                let error = Geometry::singlet(front, rear, millimeter!(5.0), circle()?)
                    .expect_err("a curvature tighter than the outline was accepted");
                assert!(error.to_string().contains("does not reach"), "{error}");
            }
            assert!(
                Geometry::cylindrical_singlet(curved, flat, millimeter!(5.0), circle()?).is_err()
            );
            let mirror = |outline| {
                Geometry::surface(
                    Isometry::identity(),
                    SurfaceShape::spherical(curved)?,
                    Isometry::identity(),
                    outline,
                )
            };
            assert!(mirror(Some(circle()?)).is_err());
            // An unbounded surface has no edge to reach.
            assert!(mirror(None).is_ok());
        }
        assert!(Geometry::singlet(millimeter!(12.5), flat, millimeter!(5.0), circle()?).is_ok());
        // A parabola opens without end, so even a short focal length reaches any edge.
        let parabola = SurfaceShape::Parabola(Parabola::new(millimeter!(1.0))?);
        assert!(
            Geometry::surface(
                Isometry::identity(),
                parabola,
                Isometry::identity(),
                Some(circle()?)
            )
            .is_ok()
        );
        Ok(())
    }
    /// A surface with a cross section ends at its edge, which is measured across the axis the cross
    /// section is extruded along: here the node's x axis, so the circle lies in the node's yz plane.
    #[test]
    fn a_surface_ends_at_its_edge_across_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?;
        assert_eq!(
            frame_of(&geometry, SurfaceGeometry::FACE)?,
            axis.append(&vertex)
        );
        for ((x, y, z), included) in [
            ((23.0, 0.0, 0.0), true),
            ((23.0, 0.0, 12.0), true),
            ((23.0, 12.0, 0.0), true),
            ((23.0, 0.0, 13.0), false),
            ((23.0, -13.0, 0.0), false),
        ] {
            assert_eq!(
                geometry
                    .kind()
                    .includes(SurfaceGeometry::FACE, &millimeter!(x, y, z)),
                included,
                "({x}, {y}, {z})"
            );
        }
        assert!(geometry.kind().window().is_none());
        Ok(())
    }
    /// Without a cross section there is nothing to end at, but the face is still stated in the axis
    /// frame.
    #[test]
    fn an_unbounded_surface_is_placed_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, None)?;
        assert_eq!(
            frame_of(&geometry, SurfaceGeometry::FACE)?,
            axis.append(&vertex)
        );
        assert!(
            geometry
                .kind()
                .includes(SurfaceGeometry::FACE, &millimeter!(23.0, 1000.0, 1000.0))
        );
        assert!(geometry.kind().window().is_none());
        Ok(())
    }
    /// A detector's cross section is a window: light outside it still passes, it is just not recorded.
    #[test]
    fn a_detector_bounds_its_window_not_its_edge() -> OpmResult<()> {
        let far = millimeter!(1000.0, 1000.0, 0.0);
        let bounded = Geometry::detector(SurfaceShape::Plane, Some(circle()?))?;
        assert_eq!(
            bounded.kind().window(),
            Some(&Rim::new(circle()?, Isometry::identity()))
        );
        assert!(bounded.kind().includes(SurfaceGeometry::FACE, &far));
        let open = Geometry::detector(SurfaceShape::Plane, None)?;
        assert!(open.kind().window().is_none());
        assert!(open.kind().includes(SurfaceGeometry::FACE, &far));
        Ok(())
    }
    /// A surface without a cross section is unbounded, a bounded surface ends at its edge and an
    /// extruded solid at the rim of its profile, stated along the extrusion axis.
    #[test]
    fn only_a_geometry_with_a_cross_section_ends_sideways() -> OpmResult<()> {
        // A point 5 mm and a point a meter off the axis, on the face at the given distance along it.
        let bounded = |geometry: Geometry, along_axis: f64| {
            (
                geometry
                    .kind()
                    .includes(FaceIndex::new(0), &millimeter!(along_axis, 5.0, 0.0)),
                geometry
                    .kind()
                    .includes(FaceIndex::new(0), &millimeter!(along_axis, 1000.0, 1000.0)),
            )
        };
        // The plane faces along z, so its points are given in the xy plane: x off the axis.
        let plane = |geometry: Geometry| {
            (
                geometry
                    .kind()
                    .includes(FaceIndex::new(0), &millimeter!(0.0, 5.0, 0.0)),
                geometry
                    .kind()
                    .includes(FaceIndex::new(0), &millimeter!(1000.0, 1000.0, 0.0)),
            )
        };
        assert_eq!(plane(Geometry::plane(None)), (true, true));
        assert_eq!(plane(Geometry::plane(Some(circle()?))), (true, false));
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Geometry::Extruded(Box::new(Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        )?));
        // Extruded along the node's x axis, the profile reaches around it in y and z; the front face
        // lies at x = 3 mm.
        assert_eq!(bounded(extruded, 3.0), (true, false));
        Ok(())
    }
    /// A hit tells where the ray meets the face, in the frame of the geometry and in that of the
    /// face, and whether a detector records it. Beyond the edge, or on a face that does not exist,
    /// there is none.
    #[test]
    fn a_ray_hits_a_face_where_the_face_reaches() -> OpmResult<()> {
        let frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        let ray_at = |x: f64| {
            Ray::new(
                frame.transform_point(&millimeter!(x, -4.0, -20.0)),
                frame.transform_vector_f64(&Vector3::z()),
                nanometer!(1053.0),
                joule!(1.0),
            )
        };
        let mirror = moved_copy(&Geometry::plane(Some(circle()?)), &frame);
        let hit = mirror
            .intersect(SurfaceGeometry::FACE, &ray_at(3.0)?)
            .ok_or_else(|| OpossumError::Other("the ray missed the mirror".into()))?;
        for (found, wanted) in hit.local_point().iter().zip([3.0, -4.0, 0.0]) {
            assert_abs_diff_eq!(found.get::<millimeter>(), wanted, epsilon = 1e-9);
        }
        let point = frame.transform_point(&hit.local_point());
        assert_abs_diff_eq!(
            (point - hit.point()).map(|c| c.value).norm(),
            0.0,
            epsilon = 1e-15
        );
        let against = -frame.transform_vector_f64(&Vector3::z());
        assert_abs_diff_eq!((hit.normal() - against).norm(), 0.0, epsilon = 1e-12);
        assert!(hit.recorded());
        // The circle reaches 12.5 mm; at (12, -4) mm, 12.6 mm out, the ray misses the mirror.
        assert!(
            mirror
                .intersect(SurfaceGeometry::FACE, &ray_at(12.0)?)
                .is_none()
        );
        assert!(mirror.intersect(FaceIndex::new(1), &ray_at(3.0)?).is_none());
        // A detector is hit there too, but does not record it.
        let detector = moved_copy(
            &Geometry::detector(SurfaceShape::Plane, Some(circle()?))?,
            &frame,
        );
        let outside = detector
            .intersect(SurfaceGeometry::FACE, &ray_at(12.0)?)
            .ok_or_else(|| OpossumError::Other("the ray missed the detector".into()))?;
        assert!(!outside.recorded());
        Ok(())
    }
    /// A ray along z, the given distance off the axis in x and y.
    fn ray_along_z(x: f64, y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(x, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    /// The edge of a surface is measured across the axis of its rim, not across its tilted face:
    /// tilted by 30°, the plane is hit at y = 4.9 mm by a ray along the node's axis, a point
    /// 4.9 / cos 30° = 5.66 mm out in the plane's own frame, but within the 5 mm rim.
    #[test]
    fn the_rim_is_measured_across_its_axis() -> OpmResult<()> {
        let tilted = Isometry::new(millimeter!(0.0, 0.0, 0.0), degree!(30.0, 0.0, 0.0))?;
        let circle = Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?;
        let plane = Geometry::surface(
            Isometry::identity(),
            SurfaceShape::Plane,
            tilted,
            Some(circle),
        )?;
        assert!(
            plane
                .intersect(SurfaceGeometry::FACE, &ray_along_z(0.0, 4.9)?)
                .is_some()
        );
        assert!(
            plane
                .intersect(SurfaceGeometry::FACE, &ray_along_z(0.0, 5.1)?)
                .is_none()
        );
        Ok(())
    }
    /// A face ends where its component does, whatever kind of component that is: the front face of
    /// this wedge-shaped solid ends where the side face cuts it, which no outline states.
    #[test]
    fn a_face_of_a_parametric_solid_is_hit_up_to_the_other_faces() -> OpmResult<()> {
        let side = Isometry::new(millimeter!(5.0, 0.0, 5.0), degree!(0.0, 90.0, 0.0))?;
        let solid = Geometry::Parametric(ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, Isometry::identity()),
                Side::Behind,
            ),
            (
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new_along_z(millimeter!(10.0))?,
                ),
                Side::InFront,
            ),
            (Face::new(SurfaceShape::Plane, side), Side::InFront),
        ])?);
        assert!(
            solid
                .intersect(FaceIndex::new(0), &ray_along_z(4.9, 0.0)?)
                .is_some()
        );
        assert!(
            solid
                .intersect(FaceIndex::new(0), &ray_along_z(5.1, 0.0)?)
                .is_none()
        );
        Ok(())
    }
    /// A sphere of 15 mm radius bounded to 12.5 mm around its axis is hit only on the sheet through
    /// its vertex: a ray tilted by 50° that reaches the sphere 135° from the vertex first, behind its
    /// equator and within the rim seen along the axis, misses it.
    #[test]
    fn a_hit_behind_the_equator_is_a_miss() -> OpmResult<()> {
        let sphere = Geometry::surface(
            Isometry::identity(),
            SurfaceShape::Sphere(Sphere::new(millimeter!(15.0))?),
            Isometry::identity(),
            Some(circle()?),
        )?;
        let (radius, polar, tilt) = (15.0, 135.0_f64.to_radians(), 50.0_f64.to_radians());
        let behind = (radius * polar.sin(), radius - radius * polar.cos());
        let direction = Vector3::new(-tilt.sin(), 0.0, tilt.cos());
        let steep = Ray::new(
            millimeter!(
                behind.0 - 30.0 * direction.x,
                0.0,
                behind.1 - 30.0 * direction.z
            ),
            direction,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        assert!(sphere.intersect(SurfaceGeometry::FACE, &steep).is_none());
        assert!(
            sphere
                .intersect(SurfaceGeometry::FACE, &ray_along_z(0.0, 5.0)?)
                .is_some()
        );
        Ok(())
    }
    /// A sphere of 1 m radius with its vertex at the origin, curving away towards +z, is hit at its
    /// vertex, with the normal against the ray; a ray running past it sideways misses it.
    #[test]
    fn a_ray_hits_a_sphere_at_its_vertex() -> OpmResult<()> {
        let sphere = Geometry::from(Face::new(
            SurfaceShape::Sphere(Sphere::new(meter!(1.0))?),
            Isometry::identity(),
        ));
        let towards = Ray::new_collimated(meter!(0.0, 0.0, -1.0), nanometer!(1000.0), joule!(1.0))?;
        let hit = sphere
            .intersect(SurfaceGeometry::FACE, &towards)
            .ok_or_else(|| OpossumError::Other("the ray missed the sphere".into()))?;
        assert!(
            hit.point().coords.map(|c| c.value).norm() < 1e-15,
            "{hit:?}"
        );
        assert!((hit.normal() + Vector3::z()).norm() < 1e-15, "{hit:?}");
        let past = Ray::new_collimated(meter!(2.0, 0.0, -1.0), nanometer!(1000.0), joule!(1.0))?;
        assert!(sphere.intersect(SurfaceGeometry::FACE, &past).is_none());
        Ok(())
    }
    /// A face on its own is an unbounded surface.
    #[test]
    fn a_face_on_its_own_is_an_unbounded_surface() -> OpmResult<()> {
        let face = Face::new(
            SurfaceShape::Plane,
            Isometry::new_along_z(millimeter!(5.0))?,
        );
        let geometry = Geometry::from(face.clone());
        assert_eq!(geometry.face(SurfaceGeometry::FACE), Some(&face));
        assert!(geometry.kind().window().is_none());
        assert!(
            geometry
                .kind()
                .includes(SurfaceGeometry::FACE, &millimeter!(1000.0, 1000.0, 5.0))
        );
        Ok(())
    }
    /// The given geometry with the given coating on its face 0.
    fn coated(mut geometry: Geometry, coating: CoatingType) -> OpmResult<Geometry> {
        geometry.surface_of_mut(FaceIndex::new(0))?.set_coating(coating);
        Ok(geometry)
    }
    /// A reflectivity of 20 %.
    fn twenty_percent() -> OpmResult<CoatingType> {
        Ok(CoatingConstantR::new(percent!(20.0))?.into())
    }
    /// Refract a bundle into vacuum at face 0 of the given component, with its ghost reflection.
    fn refract(geometry: &mut Geometry, rays: &mut Rays) -> OpmResult<Rays> {
        geometry.interact_with_rays(
            FaceIndex::new(0),
            rays,
            &Refraction(Some(&refr_index_vaccuum())),
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Stop,
        )
    }
    /// Split a bundle at face 0 of the given component with a beam splitter of the given
    /// configuration.
    fn split(
        geometry: &mut Geometry,
        rays: &mut Rays,
        config: &SplittingConfig,
    ) -> OpmResult<Rays> {
        geometry.interact_with_rays(
            FaceIndex::new(0),
            rays,
            config,
            ReflectedPart::Branch,
            MissedSurfaceStrategy::Stop,
        )
    }
    #[test]
    fn refraction_of_an_empty_bundle() -> OpmResult<()> {
        let mut rays = Rays::default();
        testing_logger::setup();
        let reflected = refract(&mut Geometry::plane(None), &mut rays)?;
        check_logs(
            log::Level::Warn,
            vec!["ray bundle contains no valid rays - not propagating"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    #[test]
    fn refraction_of_helper_rays() -> OpmResult<()> {
        let tot_energy = joule!(1.);
        let pos_strategy = Hexapolar::new(millimeter!(15.), 5)?;
        let fluence_strategy = General2DGaussian::new(
            tot_energy,
            millimeter!(0., 0.),
            millimeter!(2.5, 2.5),
            radian!(0.),
        )?;
        let mut rays = Rays::new_collimated_w_fluence_helper(
            nanometer!(1000.),
            &fluence_strategy,
            &pos_strategy,
        )?;
        refract(&mut Geometry::plane(None), &mut rays)?;
        Ok(())
    }
    /// The intended beam of a reflection continues the incoming bundle.
    #[test]
    fn a_main_reflection_continues_the_bundle() -> OpmResult<()> {
        let mut rays = Rays::default();
        let node_uuid = Uuid::new_v4();
        let parent_uuid = Uuid::new_v4();
        rays.set_node_origin_uuid(node_uuid);
        rays.set_parent_uuid(parent_uuid);
        rays.set_parent_node_split_idx(10);
        let reflected = Geometry::plane(None).interact_with_rays(
            SurfaceGeometry::FACE,
            &mut rays,
            &Refraction(Some(&refr_index_vaccuum())),
            ReflectedPart::Main,
            MissedSurfaceStrategy::Stop,
        )?;
        assert_eq!(*reflected.parent_pos_split_idx(), 10);
        assert_eq!(reflected.node_origin(), &Some(node_uuid));
        assert_eq!(reflected.parent_id(), Some(parent_uuid));
        assert_eq!(reflected.uuid(), rays.uuid());
        Ok(())
    }
    /// A ghost reflection branches off the incoming bundle where the rays hit the surface: it is a
    /// bounce of its own and starts its position history there.
    #[test]
    fn a_ghost_reflection_branches_off_the_bundle() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let reflected = refract(
            &mut coated(Geometry::plane(None), twenty_percent()?)?,
            &mut rays,
        )?;
        assert_eq!(reflected.parent_id(), Some(rays.uuid()));
        assert_eq!(*reflected.parent_pos_split_idx(), rays.ray_history_len());
        assert_eq!(reflected.bounce_lvl(), rays.bounce_lvl() + 1);
        assert_eq!(reflected.ray_history_len(), 0);
        Ok(())
    }
    #[test]
    fn an_unknown_face_is_an_error() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let error = Geometry::plane(None)
            .interact_with_rays(
                FaceIndex::new(1),
                &mut rays,
                &Refraction(None),
                ReflectedPart::Ghost,
                MissedSurfaceStrategy::Stop,
            )
            .expect_err("a face that does not exist was traced");
        assert!(error.to_string().contains("there is no face 1"), "{error}");
        Ok(())
    }
    /// A passive surface keeps the refractive index of each ray, and so its direction.
    #[test]
    fn a_passive_refraction_keeps_the_direction() -> OpmResult<()> {
        let mut rays = Rays::default();
        let mut ray0 = Ray::new(
            millimeter!(0., 0., -10.),
            vector![0.0, 1.0, 1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        ray0.set_refractive_index(1.5)?;
        let mut ray1 = Ray::new(
            millimeter!(0., 1., -10.),
            vector![0.0, 1.0, 1.0],
            nanometer!(1053.0),
            joule!(1.0),
        )?;
        ray1.set_refractive_index(2.0)?;
        rays.add_ray(ray0);
        rays.add_ray(ray1);
        Geometry::plane(None).interact_with_rays(
            SurfaceGeometry::FACE,
            &mut rays,
            &Refraction(None),
            ReflectedPart::Ghost,
            MissedSurfaceStrategy::Stop,
        )?;
        for ray in rays.iter() {
            assert_abs_diff_eq!(ray.direction(), vector![0.0, 1.0, 1.0].normalize());
        }
        Ok(())
    }
    #[test]
    fn refraction_missed() -> OpmResult<()> {
        let mut rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        testing_logger::setup();
        let reflected = refract(&mut Geometry::plane(None), &mut rays)?;
        check_logs(
            log::Level::Warn,
            vec!["rays totally reflected or missed a surface"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    #[test]
    fn refraction_energy() -> OpmResult<()> {
        let mut rays = Rays::default();
        rays.add_ray(Ray::new_collimated(
            millimeter!(0.0, 0.0, -1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let reflected = refract(
            &mut coated(Geometry::plane(None), twenty_percent()?)?,
            &mut rays,
        )?;
        assert_eq!(rays.total_energy(), joule!(0.8));
        assert_eq!(reflected.total_energy(), joule!(0.2));
        Ok(())
    }
    /// A flat surface through the origin, tilted by 45° about the y axis.
    fn tilted_splitting_plane() -> OpmResult<Geometry> {
        let iso = Isometry::new(millimeter!(0., 0., 0.), degree!(0., 45., 0.))?;
        Ok(Geometry::from(Face::new(SurfaceShape::Plane, iso)))
    }
    fn collimated_ray_along_z(wavelength: Length) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0., 0., -10.), wavelength, joule!(1.0))
    }
    fn energies(rays: &Rays) -> Vec<Energy> {
        rays.iter().map(Ray::energy).collect()
    }
    #[test]
    fn splitting_tilted() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut plane = tilted_splitting_plane()?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(reflected.total_energy().get::<joule>(), 0.4);
        let transmitted_ray = rays.iter().next().expect("a transmitted ray");
        let reflected_ray = reflected.iter().next().expect("a reflected ray");
        assert_abs_diff_eq!(transmitted_ray.direction(), Vector3::z(), epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().x.abs(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().y, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(reflected_ray.direction().z, 0.0, epsilon = 1e-12);
        assert_eq!(reflected_ray.position(), transmitted_ray.position());
        Ok(())
    }
    #[test]
    fn splitting_ignores_the_surface_coating() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut plane = coated(tilted_splitting_plane()?, twenty_percent()?)?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        assert_abs_diff_eq!(rays.total_energy().get::<joule>(), 0.6);
        assert_abs_diff_eq!(reflected.total_energy().get::<joule>(), 0.4);
        Ok(())
    }
    #[test]
    fn splitting_keeps_both_parts_intended() -> OpmResult<()> {
        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1000.0))?);
        let mut plane = tilted_splitting_plane()?;
        let reflected = split(&mut plane, &mut rays, &SplittingConfig::Ratio(0.6))?;
        // the reflected part is an intended beam, not a ghost reflection
        assert_eq!(reflected.bounce_lvl(), rays.bounce_lvl());
        // neither part loses the position history before the splitting surface
        assert_eq!(rays.ray_history_len(), 1);
        assert_eq!(reflected.ray_history_len(), 1);
        assert_ne!(reflected.uuid(), rays.uuid());
        assert!(reflected.parent_id().is_none());
        Ok(())
    }
    #[test]
    fn splitting_by_spectrum() -> OpmResult<()> {
        let spectrum: Spectrum = EdgeFilter::new(
            EdgeFilterType::ShortPass,
            nanometer!(1000.0),
            0.0..1.0,
            None,
            nanometer!(500.0)..nanometer!(1500.0),
            nanometer!(1.0),
        )?
        .into();
        let config = SplittingConfig::Spectrum(spectrum);
        let mut rays = Rays::default();
        rays.add_ray(collimated_ray_along_z(nanometer!(999.0))?);
        rays.add_ray(collimated_ray_along_z(nanometer!(1001.0))?);
        let mut plane = tilted_splitting_plane()?;
        let reflected = split(&mut plane, &mut rays, &config)?;
        assert_eq!(energies(&rays), [joule!(1.0), joule!(0.0)]);
        assert_eq!(energies(&reflected), [joule!(0.0), joule!(1.0)]);

        let mut rays = Rays::from(collimated_ray_along_z(nanometer!(1501.0))?);
        assert!(split(&mut plane, &mut rays, &config).is_err());
        Ok(())
    }
    #[test]
    fn splitting_missed() -> OpmResult<()> {
        let mut rays = Rays::from(Ray::new_collimated(
            millimeter!(0.0, 0.0, 1.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        testing_logger::setup();
        let reflected = split(
            &mut Geometry::plane(None),
            &mut rays,
            &SplittingConfig::Ratio(0.6),
        )?;
        check_logs(
            log::Level::Warn,
            vec!["rays totally reflected or missed a surface"],
        );
        assert_eq!(reflected.nr_of_rays(false), 0);
        Ok(())
    }
    /// A plane through the node, its component reaching 5 mm around the node's axis.
    fn bounded_plane() -> OpmResult<Geometry> {
        Ok(Geometry::plane(Some(Outline::new(
            CircleShape::new(millimeter!(5.0))?,
            Vec::new(),
        )?)))
    }
    /// A ray along z, the given distance off the axis in y.
    fn ray_at(y: f64) -> OpmResult<Ray> {
        Ray::new_collimated(millimeter!(0.0, y, -10.0), nanometer!(1000.0), joule!(1.0))
    }
    /// Propagate bundles forward through the passive surface of a face of a component.
    fn propagate(
        geometry: &mut Geometry,
        face: FaceIndex,
        bundles: &mut Vec<Rays>,
        config: &RayTraceConfig,
    ) -> OpmResult<()> {
        geometry.propagate_rays(
            face,
            bundles,
            &Refraction(None),
            ReflectedPart::Ghost,
            false,
            Uuid::new_v4(),
            config,
        )
    }
    /// After the bundles, the ghost reflections cached at the surface for this direction join them.
    #[test]
    fn propagation_appends_the_ghost_reflections_cached_at_the_surface() -> OpmResult<()> {
        let cached = Rays::from(vec![ray_at(0.0)?, ray_at(7.0)?]);
        let mut plane = bounded_plane()?;
        plane
            .surface_of_mut(SurfaceGeometry::FACE)?
            .set_forward_rays_cache(vec![cached.clone()]);
        let mut bundles = vec![Rays::from(vec![ray_at(4.9)?, ray_at(5.1)?])];
        propagate(
            &mut plane,
            SurfaceGeometry::FACE,
            &mut bundles,
            &RayTraceConfig::default(),
        )?;
        assert_eq!(bundles.len(), 2);
        assert_eq!(bundles[1], cached);
        Ok(())
    }
    /// A ray trace invalidates the rays the surface pass leaves below its energy threshold. The
    /// positioning run keeps them: geometry alone places the optical axis.
    #[test]
    fn a_ray_trace_drops_rays_below_the_energy_threshold() -> OpmResult<()> {
        let mut config = RayTraceConfig::default();
        config.set_min_energy_per_ray(joule!(0.5))?;
        for (config, valid) in [(config.for_positioning(), 2), (config, 1)] {
            let weak = Ray::new_collimated(
                millimeter!(0.0, 0.0, -10.0),
                nanometer!(1000.0),
                joule!(0.1),
            )?;
            let mut bundles = vec![Rays::from(vec![ray_at(0.0)?, weak])];
            propagate(
                &mut bounded_plane()?,
                SurfaceGeometry::FACE,
                &mut bundles,
                &config,
            )?;
            assert_eq!(bundles[0].nr_of_rays(true), valid);
        }
        Ok(())
    }
    /// A bundle is traced at the face it is given: the rear face of a plate here, so a ray along
    /// the axis meets it 10 mm behind the front.
    #[test]
    fn a_bundle_is_traced_at_the_face_it_is_given() -> OpmResult<()> {
        let mut plate = Geometry::singlet(
            millimeter!(f64::INFINITY),
            millimeter!(f64::INFINITY),
            millimeter!(10.0),
            Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?,
        )?;
        let mut bundles = vec![Rays::from(ray_at(0.0)?)];
        propagate(
            &mut plate,
            Extruded::REAR,
            &mut bundles,
            &RayTraceConfig::default(),
        )?;
        let ray = bundles[0]
            .iter()
            .next()
            .ok_or_else(|| OpossumError::Other("the bundle lost its ray".into()))?;
        assert_eq!(ray.position(), millimeter!(0.0, 0.0, 10.0));
        Ok(())
    }
}
