mod sdf;

use itertools::Itertools;
pub use sdf::{SDFOperation, SDF};


use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;
use crate::{ray::Ray, refractive_index::{RefractiveIndex, RefractiveIndexType}, surface::{GeoSurf, GeoSurface, OpticalSurface}, utils::geom_transformation::Isometry};

/// This struct represents an optical component, which consists of a geometric volume and a given isometry. 
/// properties such as the [`CoatingType`].
pub struct OpticComponent {
    volume: OpticalVolume, 
    isometry: Isometry
}

//Struct to store a collection of surface objects. This also includes [`SurfaceCombination`] structs
/// To combine several surface objects, an [`SDFOperation`], such as Union, Intersection or Subtraction, must be defined
#[derive(Clone)]
pub struct OpticalVolume {
    surf_objs: Vec<OpticalSurface>,
    sdf_op: SDFOperation,
    // bbox: tessellation::BoundingBox<f64>,
    refractive_index: RefractiveIndexType,
    isometry: Isometry
}


impl GeoSurface for OpticalVolume{
    fn calc_intersect_and_normal_do(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        todo!()
    }

    fn isometry(&self) -> &Isometry {
        &self.isometry
    }

    fn set_isometry(&mut self, isometry: &Isometry) {
        self.isometry = isometry.clone()
    }
    
    fn calc_intersections(&self, ray: &Ray) -> Vec<Point3<Length>> {
        let transformed_ray = ray.inverse_transformed_ray(self.isometry());
        self.surf_objs.iter().map(|s| s.geo_surface().calc_intersections(&transformed_ray)).flatten().collect_vec()
    }
}

// impl Render<'_> for SurfaceCombination {}
// impl Renderable<'_> for SurfaceCombination {}

impl SDF for OpticalVolume {
    fn sdf_eval_point(&self, p: &Point3<f64>) -> f64 {
        self.sdf_eval(p)
    }
    // fn sdf_eval_with_color(&self, p: &Point3<f64>) -> (f64, Vector3<f64>) {
    //     (self.sdf_eval_point(p), self.get_color(p))
    // }
}
impl OpticalVolume {
    /// Create a new [`SDFCollection`] struct
    /// # Attributes
    /// - `surf_objs`: vector of sdf objects: must hav implemented the GeoSurface trait
    /// - `sdf_op_opt`: option for the Operation that should be used for combining the sdfs. default: Union
    /// Returns
    /// - None if the length of the SDF oject vector is zero
    /// - `Option<Self>` otherwise
    #[must_use]
    pub fn new(
        surf_objs: Vec<OpticalSurface>,
        sdf_op_opt: Option<SDFOperation>,
        refractive_index: RefractiveIndexType,
        // bbox: tessellation::BoundingBox<f64>,
        isometry: Isometry
    ) -> Option<Self> {
        if surf_objs.is_empty() {
            None
        } else if let Some(sdf_op) = sdf_op_opt {
            Some(Self {
                surf_objs: surf_objs,
                sdf_op,
                refractive_index,
                // bbox,
                isometry
            })
        } else {
            Some(Self {
                surf_objs: surf_objs,
                sdf_op: SDFOperation::Union,
                refractive_index,
                // bbox,
                isometry
            })
        }
    }
    /// Returns the bounding box of this collection
    // #[must_use]
    // pub const fn bounding_box(&self) -> &tessellation::BoundingBox<f64> {
    //     &self.bbox
    // }
    /// Add and sdf object (must implement GeoSurface) to this [`SDFCollection`]
    pub fn add_surf_obj(&mut self, surf_obj: OpticalSurface) {
        self.surf_objs.push(surf_obj.clone());
    }

    /// Evaluate the sdf of this [`SDFCollection`] for a given point
    /// # Attributes
    /// - `p`: Point3 at which the sdf should be evaluated
    /// # Returns
    /// Retruns the smallest signed distance to this point
    #[must_use]
    pub fn sdf_eval(&self, p: &Point3<f64>) -> f64 {
        match self.sdf_op {
            SDFOperation::Intersection => self.sdf_intersection(p),
            SDFOperation::Union => self.sdf_union(p),
            SDFOperation::Subtraction { idx } => self.sdf_subtraction(p, idx),
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as Union for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this Union
    #[must_use]
    pub fn sdf_union(&self, p: &Point3<f64>) -> f64 {
        if self.surf_objs.len() > 1 {
            let surfaces = self.surf_objs[1..].iter().map(|s| s.geo_surface()).collect::<Vec<&GeoSurf>>();
            self.surf_objs[0].geo_surface().sdf_union_point(surfaces.as_slice(), p)
        } else {
            self.surf_objs[0].geo_surface().sdf_eval_point(p)
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as intersection for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this intersection
    #[must_use]
    pub fn sdf_intersection(&self, p: &Point3<f64>) -> f64 {
        if self.surf_objs.len() > 1 {
            let surfaces = self.surf_objs[1..].iter().map(|s| s.geo_surface()).collect::<Vec<&GeoSurf>>();
            self.surf_objs[0].sdf_intersection_point(surfaces.as_slice(), p)
        } else {
            self.surf_objs[0].sdf_eval_point(p)
        }
    }

    /// Evaluate the sdf of this [`SDFCollection`] as subtraction for a given point
    /// # Attributes
    /// - `p`: &Point3 at which the sdf should be evaluated
    /// # Returns
    /// Returns the signed distance to this subtraction
    #[must_use]
    pub fn sdf_subtraction(&self, p: &Point3<f64>, subtract_from_idx: usize) -> f64 {
        if self.surf_objs.len() > 1 {
            if subtract_from_idx == self.surf_objs.len() - 1 {
                let surfaces = self.surf_objs[..subtract_from_idx].iter().map(|s| s.geo_surface()).collect::<Vec<&GeoSurf>>();

                self.surf_objs[subtract_from_idx]
                    .sdf_subtraction_point(surfaces.as_slice(), p)
            } else {

                let surfaces_prev = self.surf_objs[0..subtract_from_idx].iter().map(|s| s.geo_surface()).collect::<Vec<&GeoSurf>>();
                let surfaces_post = self.surf_objs[subtract_from_idx + 1..].iter().map(|s| s.geo_surface()).collect::<Vec<&GeoSurf>>();
                self.surf_objs[subtract_from_idx].sdf_subtraction_point(
                    &[
                        surfaces_prev.as_slice(),
                        surfaces_post.as_slice()
                    ]
                    .concat(),
                    p,
                )
            }
        } else {
            self.surf_objs[0].sdf_eval_point(p)
        }
    }
}