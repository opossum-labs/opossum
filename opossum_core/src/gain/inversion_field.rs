#![warn(missing_docs)]
//! The population inversion of an active medium, discretised over the [`Body`] it fills.
//!
//! A pumped medium is not pumped uniformly: the inversion varies across the volume, and every pass
//! of a beam draws it down where that beam actually went. An [`InversionField`] is where that state
//! lives. It is the single interface between the two sides of the amplification: whatever pumps the
//! medium writes into it, and whatever [`GainModel`](super::GainModel) extracts energy from the
//! medium reads out of it. Neither side has to know the other, and the field survives between
//! passes, which is what makes a multi-pass amplifier more than a repeated single pass.
//!
//! **What a cell holds** is the absolute population density of the upper laser level, as a
//! [`VolumetricNumberDensity`]. A relative inversion would be the more convenient number to hand a
//! gain formula, but it is a ratio to a dopant density that
//! [`Material`](crate::material::Material) does not carry today — an absolute density needs nothing
//! but itself and can be converted once the material data exists. One level is stored, which is the
//! ideal four level system; further levels are further fields, not a different type.
//!
//! **The grid** is Cartesian and lives entirely in the optic's own frame, so it follows the
//! component when that is moved or tilted instead of sampling it on a staircase. Round edges do get
//! stair-stepped transversally, which is the accepted price of a Cartesian grid over an arbitrary
//! cross section: each cell is either in the medium or not, and [`InversionField::is_inside`] says
//! which.
//!
//! *Entirely* in the optic's frame means the field stores **nothing about where that optic is**.
//! Where a node stands is the node's own business, kept in its
//! [`isometry`](crate::core_optics::NodeAttr::isometry) and its alignment, and a copy of it in a
//! field that outlives a single pass would silently go stale the moment the node is moved. Instead
//! everything here — the mask, the bounds, the populations — stays valid under any placement, and
//! [`InversionField::cell_at`] takes its point already expressed in the optic's frame.
//!
//! The grid is built **once**, in [`InversionField::from_body`]: that is where all the geometry is
//! evaluated, so every later read or write is a plain index into an owned, mutable array rather than
//! a fresh query against the body.

use crate::{
    error::{OpmResult, OpossumError},
    geometry::body::{Body, BoundingBox},
    num_per_m3,
    utils::math_utils::{to_f64, try_f64_to_usize},
};
use nalgebra::{DMatrix, Point3};
use std::ops::Range;
use uom::si::f64::{Length, Volume, VolumetricNumberDensity};

/// The index of one cell of an [`InversionField`], counted along x, y and z.
pub type CellIndex = (usize, usize, usize);

/// The inversion of an active medium, sampled on a grid over the medium's [`Body`].
///
/// See the [module documentation](self) for what a cell holds and which frame the grid lives in.
#[derive(Debug, Clone, PartialEq)]
pub struct InversionField {
    /// population density of the upper laser level: one transversal plane per longitudinal step
    slices: Vec<DMatrix<VolumetricNumberDensity>>,
    /// which cells of each plane lie within the body
    inside: Vec<DMatrix<bool>>,
    /// the extent the grid spans, in the optic's frame — the body's own bounding box
    bounds: BoundingBox,
}

impl InversionField {
    /// Lay a grid of the given size over the given [`Body`] and start it out unpumped.
    ///
    /// This is the one place geometry is evaluated: the body's
    /// [`bounding_box`](Body::bounding_box) gives the domain, and every cell center is tested
    /// against [`contains`](Body::contains) once to record whether it holds medium at all. Every
    /// later access is a plain index.
    ///
    /// The resolution is an argument rather than a property of the node the body came from: nothing
    /// reads the field yet, so there is nobody to configure it for.
    ///
    /// # Arguments
    ///
    /// - `body`: the volume the field is defined over
    /// - `dimensions`: the number of cells along the body's x, y and z axis
    ///
    /// # Returns
    ///
    /// A field spanning the body's bounding box, with every cell at zero population density.
    ///
    /// # Errors
    ///
    /// This function returns an error if any of the three dimensions is zero — a grid without cells
    /// has no extent to interpret its ranges against — or if the body cannot state its bounding box
    /// or answer where it is.
    pub fn from_body(body: &dyn Body, dimensions: CellIndex) -> OpmResult<Self> {
        let (nx, ny, nz) = dimensions;
        if nx == 0 || ny == 0 || nz == 0 {
            return Err(OpossumError::Other(
                "an inversion field needs at least one cell along each of its axes".into(),
            ));
        }
        let bounds = body.bounding_box()?;
        // Only needed to ask the body where it is - the grid that comes out of it is expressed in
        // the body's own frame and does not keep the placement around.
        let placement = body.isometry();
        let (x_range, y_range, z_range) = (bounds.x_range(), bounds.y_range(), bounds.z_range());
        let mut slices = Vec::with_capacity(nz);
        let mut inside = Vec::with_capacity(nz);
        for k in 0..nz {
            let z = cell_center(&z_range, nz, k);
            // `DMatrix::from_vec` reads its data column by column, and one column of a transversal
            // plane collects the cells sharing an x position. This is the layout `FluenceData` uses
            // for its own transversal field, where the columns count x and the rows count y.
            let mut mask = Vec::with_capacity(nx * ny);
            for i in 0..nx {
                let x = cell_center(&x_range, nx, i);
                for j in 0..ny {
                    let y = cell_center(&y_range, ny, j);
                    let center = placement.transform_point(&Point3::new(x, y, z));
                    mask.push(body.contains(&center)?);
                }
            }
            inside.push(DMatrix::from_vec(ny, nx, mask));
            slices.push(DMatrix::from_element(ny, nx, num_per_m3!(0.0)));
        }
        Ok(Self {
            slices,
            inside,
            bounds,
        })
    }
    /// Return the extent this [`InversionField`] spans.
    ///
    /// This is the body's own [`bounding_box`](Body::bounding_box), kept as the grid was laid out
    /// over it, and it is stated in the optic's frame like everything else here. Anything evaluating
    /// a profile over the medium needs it: a distribution that decays from a face of the body has to
    /// know where that face is.
    ///
    /// # Returns
    ///
    /// The box the grid covers.
    #[must_use]
    pub const fn bounds(&self) -> BoundingBox {
        self.bounds
    }
    /// Return the number of cells of this [`InversionField`] along its x, y and z axis.
    ///
    /// # Returns
    ///
    /// The size of the grid, which is what was asked of [`InversionField::from_body`].
    #[must_use]
    pub fn dimensions(&self) -> CellIndex {
        self.slices.first().map_or((0, 0, 0), |slice| {
            (slice.ncols(), slice.nrows(), self.slices.len())
        })
    }
    /// Return the volume of a single cell of this [`InversionField`].
    ///
    /// All cells are the same size, so this is what turns a population density into a number of
    /// excited ions — the step any energy balance over the medium starts from.
    ///
    /// # Returns
    ///
    /// The volume one cell occupies.
    #[must_use]
    pub fn cell_volume(&self) -> Volume {
        let (nx, ny, nz) = self.dimensions();
        extent(&self.bounds.x_range()) / to_f64(nx) * extent(&self.bounds.y_range()) / to_f64(ny)
            * extent(&self.bounds.z_range())
            / to_f64(nz)
    }
    /// Return whether the given cell holds medium at all.
    ///
    /// The grid spans the body's bounding box, which is larger than the body itself wherever that is
    /// not a box: the cells outside it exist, but there is nothing there to excite.
    ///
    /// # Arguments
    ///
    /// - `cell`: the cell to be tested
    ///
    /// # Returns
    ///
    /// `true` if the center of the cell lies inside the body, `false` if it does not or if the cell
    /// is not part of the grid at all.
    #[must_use]
    pub fn is_inside(&self, cell: CellIndex) -> bool {
        let (i, j, k) = cell;
        self.inside
            .get(k)
            .and_then(|slice| slice.get((j, i)))
            .copied()
            .unwrap_or(false)
    }
    /// Return the upper laser level population density stored in the given cell.
    ///
    /// # Arguments
    ///
    /// - `cell`: the cell to be read
    ///
    /// # Returns
    ///
    /// The population density of the cell, or `None` if it is not part of the grid. A cell outside
    /// the body reads as the zero it was initialised to — ask [`InversionField::is_inside`] to tell
    /// the two apart.
    #[must_use]
    pub fn population(&self, cell: CellIndex) -> Option<VolumetricNumberDensity> {
        let (i, j, k) = cell;
        self.slices.get(k)?.get((j, i)).copied()
    }
    /// Write the upper laser level population density of the given cell.
    ///
    /// # Arguments
    ///
    /// - `cell`: the cell to be written
    /// - `population`: the population density to store
    ///
    /// # Errors
    ///
    /// This function returns an error if the cell is not part of the grid. Writing a cell that lies
    /// outside the body is *not* an error: whether a cell carries medium is a question for
    /// [`InversionField::is_inside`], and a producer sweeping a region should not have to be right
    /// about the body's outline to be allowed to write.
    pub fn set_population(
        &mut self,
        cell: CellIndex,
        population: VolumetricNumberDensity,
    ) -> OpmResult<()> {
        let (i, j, k) = cell;
        let Some(entry) = self
            .slices
            .get_mut(k)
            .and_then(|slice| slice.get_mut((j, i)))
        else {
            return Err(OpossumError::Other(format!(
                "the cell {cell:?} is not part of an inversion field of the size {:?}",
                self.dimensions()
            )));
        };
        *entry = population;
        Ok(())
    }
    /// Return the cell the given point falls into.
    ///
    /// This is what connects the grid back to the rest of the simulation: a ray hits the medium
    /// somewhere, and this says which cell that somewhere is.
    ///
    /// # Arguments
    ///
    /// - `local_point`: the point to be located, given **in the frame of the optic** the medium
    ///   belongs to. A caller holding a global point — a ray position, say — inverse transforms it
    ///   by the node's [`effective_node_iso`](crate::core_optics::OpticNodeExt::effective_node_iso)
    ///   first. The field does not do that itself on purpose: it would have to keep a copy of a
    ///   placement it does not own and cannot notice changing.
    ///
    /// # Returns
    ///
    /// The index of the cell containing the point, or `None` if the point lies outside the grid.
    /// Cells are half-open, so a point on the upper boundary of the grid belongs to no cell, the
    /// same way [`Body::contains`] treats the exit boundary of a body.
    #[must_use]
    pub fn cell_at(&self, local_point: &Point3<Length>) -> Option<CellIndex> {
        let (nx, ny, nz) = self.dimensions();
        let index = |position: Length, range: &Range<Length>, count: usize| {
            let cells_from_start = ((position - range.start) / extent(range)).value * to_f64(count);
            let index = try_f64_to_usize(cells_from_start.floor())?;
            (index < count).then_some(index)
        };
        Some((
            index(local_point.x, &self.bounds.x_range(), nx)?,
            index(local_point.y, &self.bounds.y_range(), ny)?,
            index(local_point.z, &self.bounds.z_range(), nz)?,
        ))
    }
    /// Return the center of the given cell.
    ///
    /// The exact inverse of [`InversionField::cell_at`], and the direction anything writing into the
    /// field needs: a pump profile is a function of position, so it has to be told where the cell it
    /// is about to fill actually sits. The center is what the cell was masked by in
    /// [`InversionField::from_body`], so a profile samples the medium exactly where the mask did.
    ///
    /// # Arguments
    ///
    /// - `cell`: the cell to locate
    ///
    /// # Returns
    ///
    /// The center of the cell **in the frame of the optic** the medium belongs to — the same frame
    /// [`InversionField::cell_at`] expects its point in — or `None` if the cell is not part of the
    /// grid.
    #[must_use]
    pub fn cell_center(&self, cell: CellIndex) -> Option<Point3<Length>> {
        let (nx, ny, nz) = self.dimensions();
        let (i, j, k) = cell;
        if i >= nx || j >= ny || k >= nz {
            return None;
        }
        Some(Point3::new(
            cell_center(&self.bounds.x_range(), nx, i),
            cell_center(&self.bounds.y_range(), ny, j),
            cell_center(&self.bounds.z_range(), nz, k),
        ))
    }
}

/// Return how far the given range reaches.
///
/// # Arguments
///
/// - `range`: the range to be measured
///
/// # Returns
///
/// The distance between the start and the end of the range.
fn extent(range: &Range<Length>) -> Length {
    range.end - range.start
}

/// Return the center of one cell of an evenly divided range.
///
/// # Arguments
///
/// - `range`: the range covered by all cells together
/// - `count`: the number of cells the range is divided into
/// - `index`: the index of the cell in question
///
/// # Returns
///
/// The position of the center of the cell.
fn cell_center(range: &Range<Length>, count: usize, index: usize) -> Length {
    range.start + extent(range) * ((to_f64(index) + 0.5) / to_f64(count))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        core_optics::{OpticNode, volumetric::Volumetric},
        degree,
        geometry::{Plane, body::SurfaceBoundedBody, geo_surface::GeoSurfaceRef},
        millimeter,
        nodes::Lens,
        num_per_cm3,
        types::validated_type_definitions::ValidatedCrossSection,
        utils::geom_transformation::Isometry,
    };
    use approx::assert_abs_diff_eq;
    use std::{
        f64::consts::PI,
        sync::{Arc, Mutex},
    };

    /// Create a disk of the given thickness and radius, placed by the given isometry.
    fn disk(
        thickness: Length,
        radius: Length,
        placement: Isometry,
    ) -> OpmResult<SurfaceBoundedBody> {
        Ok(SurfaceBoundedBody::new(
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(placement)))),
            GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(
                placement.append(&Isometry::new_along_z(thickness)?),
            )))),
            ValidatedCrossSection::try_new(Aperture::new_circle(
                radius,
                ApertureType::Hole,
                None,
            )?)?,
            placement,
        ))
    }
    /// Iterate over every cell of a field of the given size.
    fn all_cells(dimensions: CellIndex) -> impl Iterator<Item = CellIndex> {
        let (nx, ny, nz) = dimensions;
        (0..nx).flat_map(move |i| (0..ny).flat_map(move |j| (0..nz).map(move |k| (i, j, k))))
    }
    /// Return the volume of all cells of the given field that hold medium.
    fn covered_volume(field: &InversionField) -> Volume {
        let covered = all_cells(field.dimensions())
            .filter(|cell| field.is_inside(*cell))
            .count();
        field.cell_volume() * to_f64(covered)
    }
    #[test]
    fn the_grid_has_the_requested_shape() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let field = InversionField::from_body(&body, (7, 5, 3))?;
        assert_eq!(field.dimensions(), (7, 5, 3));
        // The disk fills its own bounding box in z and spans twice its radius transversally, so
        // the cells come out as that box divided by the requested number of them.
        assert_abs_diff_eq!(
            field.cell_volume().value,
            (millimeter!(10.0) / 7.0 * millimeter!(10.0) / 5.0 * millimeter!(10.0) / 3.0).value,
            epsilon = 1e-18
        );
        Ok(())
    }
    #[test]
    fn a_grid_needs_at_least_one_cell_per_axis() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        for dimensions in [(0, 4, 4), (4, 0, 4), (4, 4, 0)] {
            assert!(InversionField::from_body(&body, dimensions).is_err());
        }
        assert!(InversionField::from_body(&body, (1, 1, 1)).is_ok());
        Ok(())
    }
    #[test]
    fn the_grid_spans_the_bounding_box_of_the_body() -> OpmResult<()> {
        // The field keeps the box it was laid out over, so a profile can ask where a face of the
        // medium is without going back to the body it came from.
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let field = InversionField::from_body(&body, (4, 4, 4))?;
        assert_eq!(field.bounds(), body.bounding_box()?);
        Ok(())
    }
    #[test]
    fn every_cell_center_maps_back_to_its_own_cell() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let dimensions = (7, 5, 3);
        let field = InversionField::from_body(&body, dimensions)?;
        for cell in all_cells(dimensions) {
            let center = field
                .cell_center(cell)
                .ok_or_else(|| OpossumError::Other(format!("cell {cell:?} has no center")))?;
            assert_eq!(field.cell_at(&center), Some(cell));
        }
        // Beyond the grid there is no cell, on either side and on every axis. The upper boundary
        // belongs to no cell either, the same way a body does not claim its exit surface.
        assert_eq!(field.cell_at(&millimeter!(0.0, 0.0, -0.1)), None);
        assert_eq!(field.cell_at(&millimeter!(0.0, 0.0, 10.0)), None);
        assert_eq!(field.cell_at(&millimeter!(5.1, 0.0, 5.0)), None);
        assert_eq!(field.cell_at(&millimeter!(0.0, -5.1, 5.0)), None);
        Ok(())
    }
    #[test]
    fn a_cell_center_sits_where_the_grid_puts_it() -> OpmResult<()> {
        // The disk spans -5..5 mm transversally and 0..10 mm in z, so with four cells per axis the
        // first cell is centered an eighth of the way in on each of them.
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let field = InversionField::from_body(&body, (4, 4, 4))?;
        let center = field
            .cell_center((0, 0, 0))
            .ok_or_else(|| OpossumError::Other("the first cell has no center".into()))?;
        for (found, expected) in [
            (center.x, millimeter!(-3.75)),
            (center.y, millimeter!(-3.75)),
            (center.z, millimeter!(1.25)),
        ] {
            assert_abs_diff_eq!(found.value, expected.value, epsilon = 1e-15);
        }
        // Cells outside the grid have no center, on every axis.
        for cell in [(4, 0, 0), (0, 4, 0), (0, 0, 4)] {
            assert_eq!(field.cell_center(cell), None);
        }
        Ok(())
    }
    #[test]
    fn the_mask_follows_the_cross_section() -> OpmResult<()> {
        // The grid spans the box around the disk, so its transversal corners stick out of the round
        // cross section while its center is well within it.
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let field = InversionField::from_body(&body, (8, 8, 4))?;
        assert!(field.is_inside((3, 3, 0)));
        assert!(field.is_inside((4, 4, 3)));
        for corner in [(0, 0, 0), (7, 0, 0), (0, 7, 0), (7, 7, 3)] {
            assert!(
                !field.is_inside(corner),
                "cell {corner:?} should be outside"
            );
        }
        // A cell that is not part of the grid at all holds no medium either.
        assert!(!field.is_inside((8, 0, 0)));
        Ok(())
    }
    #[test]
    fn the_covered_volume_converges_to_the_body_volume() -> OpmResult<()> {
        // The test that the grid really covers the right domain: a disk fills pi/4 of its own
        // bounding box, and the staircase the Cartesian grid cuts around its rim has to vanish as
        // the cells get smaller.
        let (radius, thickness) = (millimeter!(5.0), millimeter!(10.0));
        let body = disk(thickness, radius, Isometry::identity())?;
        let exact = PI * radius * radius * thickness;
        let error = |cells: usize| -> OpmResult<f64> {
            let field = InversionField::from_body(&body, (cells, cells, 2))?;
            Ok(((covered_volume(&field) - exact) / exact).value.abs())
        };
        // Note that the error does not fall monotonically with every refinement: which cell centers
        // happen to land inside the rim is an arithmetic accident of the resolution, and doubling it
        // can reproduce the very same count. The two resolutions compared here are therefore far
        // enough apart for the trend to show rather than adjacent.
        let (coarse, fine) = (error(8)?, error(64)?);
        assert!(coarse < 0.05, "coarse grid is off by {coarse}");
        assert!(
            fine < 0.5 * coarse,
            "refining the grid did not help: {coarse} -> {fine}"
        );
        assert!(fine < 0.01, "fine grid is off by {fine}");
        Ok(())
    }
    #[test]
    fn the_grid_follows_the_frame_of_the_optic() -> OpmResult<()> {
        // Moving and tilting the component must not change the field at all: the grid is laid out
        // in the component's own frame, and the field keeps no record of where that frame is. This
        // is what lets it outlive a pass without going stale when the node is realigned.
        let placement = Isometry::new(
            millimeter!(20.0, -5.0, 100.0),
            Point3::new(degree!(15.0), degree!(-10.0), degree!(30.0)),
        )?;
        let dimensions = (8, 8, 3);
        let upright = InversionField::from_body(
            &disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?,
            dimensions,
        )?;
        let placed = InversionField::from_body(
            &disk(millimeter!(10.0), millimeter!(5.0), placement)?,
            dimensions,
        )?;
        for cell in all_cells(dimensions) {
            assert_eq!(
                upright.is_inside(cell),
                placed.is_inside(cell),
                "cell {cell:?} changed when the component was moved"
            );
        }
        // ... and a point of the medium, stated in that frame, lands in the same cell either way
        let on_the_axis = millimeter!(0.0, 0.0, 5.0);
        assert_eq!(upright.cell_at(&on_the_axis), placed.cell_at(&on_the_axis));
        assert!(upright.cell_at(&on_the_axis).is_some());
        Ok(())
    }
    #[test]
    fn a_population_can_be_written_and_read_back() -> OpmResult<()> {
        let body = disk(millimeter!(10.0), millimeter!(5.0), Isometry::identity())?;
        let mut field = InversionField::from_body(&body, (4, 4, 2))?;
        // A fresh field is unpumped everywhere.
        assert!(all_cells((4, 4, 2)).all(|cell| field.population(cell) == Some(num_per_m3!(0.0))));
        let population = num_per_cm3!(1.0e19);
        field.set_population((1, 2, 1), population)?;
        assert_eq!(field.population((1, 2, 1)), Some(population));
        // ... and only that one cell changed
        assert_eq!(field.population((2, 1, 1)), Some(num_per_m3!(0.0)));
        // Cells outside the grid can neither be read nor written.
        assert_eq!(field.population((4, 0, 0)), None);
        assert_eq!(field.population((0, 0, 2)), None);
        assert!(field.set_population((0, 0, 2), population).is_err());
        Ok(())
    }
    #[test]
    fn a_field_over_the_volume_of_a_lens() -> OpmResult<()> {
        // The whole point of the exercise: the domain comes from a real component, through the
        // body its own surfaces and clear aperture enclose.
        let mut lens = Lens::default();
        lens.set_isometry(Isometry::identity())?;
        let body = lens.volume_body()?;
        let field = InversionField::from_body(&body, (9, 9, 5))?;
        assert_eq!(field.dimensions(), (9, 9, 5));
        // the axis of the lens runs through the middle column of cells, which is all medium
        assert!((0..5).all(|k| field.is_inside((4, 4, k))));
        // ... while the corners of the box are outside the round clear aperture
        assert!(!field.is_inside((0, 0, 2)));
        Ok(())
    }
}
