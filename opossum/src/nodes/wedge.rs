use super::NodeAttr;
use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    joule,
    lightdata::LightData,
    millimeter, nanometer,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::Proptype,
    ray::Ray,
    refractive_index::{refr_index_vaccuum, RefrIndexConst, RefractiveIndex, RefractiveIndexType},
    surface::Plane,
    utils::{geom_transformation::Isometry, EnumProxy},
};
use nalgebra::{Point3, Vector3};
use num::Zero;
use uom::si::f64::{Angle, Length};

#[derive(Debug)]
/// An optical element with two flat surfaces, a given thickness and a  given wedge angle (= wedged window).
///
///
/// ## Optical Ports
///   - Inputs
///     - `front`
///   - Outputs
///     - `rear`
///
/// ## Properties
///   - `name`
///   - `inverted`
///   - `center thickness`
///   - `refractive index`
///   - `wedge`
pub struct Wedge {
    node_attr: NodeAttr,
}
impl Default for Wedge {
    /// Create a wedge with a center thickness of 10.0 mm, refractive index of 1.5 and no wedge angle (flat windows)
    fn default() -> Self {
        let mut node_attr = NodeAttr::new("wedge", "wedge");
        node_attr
            .create_property(
                "center thickness",
                "thickness of the lens in the center",
                None,
                millimeter!(10.0).into(),
            )
            .unwrap();
        node_attr
            .create_property(
                "refractive index",
                "refractive index of the lens material",
                None,
                EnumProxy::<RefractiveIndexType> {
                    value: RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap()),
                }
                .into(),
            )
            .unwrap();
        node_attr
            .create_property("wedge", "wedge angle", None, Angle::zero().into())
            .unwrap();
        let mut ports = OpticPorts::new();
        ports.create_input("front").unwrap();
        ports.create_output("rear").unwrap();
        node_attr.set_property("apertures", ports.into()).unwrap();
        Self { node_attr }
    }
}
impl Wedge {
    /// Create a new wedge.
    ///
    /// # Errors
    ///
    /// This function will return an error if .
    pub fn new(
        name: &str,
        center_thickness: Length,
        wedge_angle: Angle,
        refractive_index: &dyn RefractiveIndex,
    ) -> OpmResult<Self> {
        let mut wedge = Self::default();
        wedge.node_attr.set_property("name", name.into())?;
        if center_thickness.is_sign_negative() || !center_thickness.is_finite() {
            return Err(crate::error::OpossumError::Other(
                "center thickness must be positive and finite".into(),
            ));
        }
        wedge
            .node_attr
            .set_property("center thickness", center_thickness.into())?;

        wedge.node_attr.set_property(
            "refractive index",
            EnumProxy::<RefractiveIndexType> {
                value: refractive_index.to_enum(),
            }
            .into(),
        )?;
        if !wedge_angle.is_finite() {
            return Err(crate::error::OpossumError::Other(
                "wedge angle must be finite".into(),
            ));
        }
        wedge.node_attr.set_property("wedge", wedge_angle.into())?;
        Ok(wedge)
    }
}

impl Optical for Wedge {
    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        let Some(data) = incoming_data.get("front") else {
            return Ok(LightResult::default());
        };
        let light_data = match analyzer_type {
            AnalyzerType::Energy => data.clone(),
            AnalyzerType::RayTrace(_) => {
                if let LightData::Geometric(mut rays) = data.clone() {
                    let Ok(Proptype::RefractiveIndex(index_model)) =
                        self.node_attr.get_property("refractive index")
                    else {
                        return Err(OpossumError::Analysis(
                            "cannot read refractive index".into(),
                        ));
                    };
                    if let Some(iso) = self.effective_iso() {
                        let plane = Plane::new(&iso);
                        rays.refract_on_surface(&plane, &index_model.value)?;

                        if let Some(aperture) = self.ports().input_aperture("front") {
                            rays.apodize(aperture)?;
                            if let AnalyzerType::RayTrace(config) = analyzer_type {
                                rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                            }
                        } else {
                            return Err(OpossumError::OpticPort("input aperture not found".into()));
                        };
                        let Ok(Proptype::Length(center_thickness)) =
                            self.node_attr.get_property("center thickness")
                        else {
                            return Err(OpossumError::Analysis(
                                "cannot read center thickness".into(),
                            ));
                        };
                        let Ok(Proptype::Angle(angle)) = self.node_attr.get_property("wedge")
                        else {
                            return Err(OpossumError::Analysis("cannot wedge angle".into()));
                        };
                        let thickness_iso = Isometry::new_along_z(*center_thickness)?;
                        let wedge_iso = Isometry::new(
                            Point3::origin(),
                            Point3::new(*angle, Angle::zero(), Angle::zero()),
                        )?;
                        let isometry = iso.append(&thickness_iso).append(&wedge_iso);
                        rays.set_refractive_index(&index_model.value)?;
                        let plane = Plane::new(&isometry);
                        rays.refract_on_surface(&plane, &refr_index_vaccuum())?;
                    } else {
                        return Err(OpossumError::Analysis(
                            "no location for surface defined. Aborting".into(),
                        ));
                    }
                    if let Some(aperture) = self.ports().output_aperture("rear") {
                        rays.apodize(aperture)?;
                        if let AnalyzerType::RayTrace(config) = analyzer_type {
                            rays.invalidate_by_threshold_energy(config.min_energy_per_ray())?;
                        }
                    } else {
                        return Err(OpossumError::OpticPort("ouput aperture not found".into()));
                    };
                    LightData::Geometric(rays)
                } else {
                    return Err(OpossumError::Analysis(
                        "expected ray data at input port".into(),
                    ));
                }
            }
        };
        let light_result = LightResult::from([("rear".into(), light_data)]);
        Ok(light_result)
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.node_attr.set_property(name, prop)
    }
    fn node_attr(&self) -> &NodeAttr {
        &self.node_attr
    }
    fn set_isometry(&mut self, isometry: Isometry) {
        self.node_attr.set_isometry(isometry);
    }
    fn output_port_isometry(&self, _output_port_name: &str) -> Option<Isometry> {
        // if wedge is aligned (tilted, decentered), calculate single ray on incoming optical axis
        // todo: use central wavelength
        let alignment_iso = self
            .node_attr
            .alignment()
            .clone()
            .unwrap_or_else(|| Isometry::identity());
        let mut ray =
            Ray::new_collimated(millimeter!(0.0, 0.0, -1.0), nanometer!(1000.0), joule!(1.0))
                .unwrap();
        let front_plane = Plane::new(&alignment_iso);
        let Ok(Proptype::RefractiveIndex(index_model)) =
            self.node_attr.get_property("refractive index")
        else {
            return None;
        };
        let n2 = index_model
            .value
            .get_refractive_index(ray.wavelength())
            .unwrap();
        ray.refract_on_surface(&front_plane, n2).unwrap();
        let Ok(Proptype::Length(center_thickness)) =
            self.node_attr.get_property("center thickness")
        else {
            return None;
        };
        let Ok(Proptype::Angle(angle)) = self.node_attr.get_property("wedge") else {
            return None;
        };
        let thickness_iso = Isometry::new_along_z(*center_thickness).unwrap();
        let wedge_iso = Isometry::new(
            Point3::origin(),
            Point3::new(*angle, Angle::zero(), Angle::zero()),
        )
        .unwrap();
        let isometry = alignment_iso.append(&thickness_iso).append(&wedge_iso);
        let back_plane = Plane::new(&isometry);
        let n2 = refr_index_vaccuum()
            .get_refractive_index(ray.wavelength())
            .unwrap();
        ray.refract_on_surface(&back_plane, n2).unwrap();
        let alignment_iso = Isometry::new_from_view(ray.position(), ray.direction(), Vector3::y());
        if let Some(iso) = self.node_attr.isometry() {
            // println!("wedge output axis: {:?}", iso.append(&alignment_iso));
            Some(iso.append(&alignment_iso))
        } else {
            None
        }
    }
}
impl Dottable for Wedge {
    fn node_color(&self) -> &str {
        "aquamarine"
    }
}
