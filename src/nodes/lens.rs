use crate::{
    analyzer::AnalyzerType,
    dottable::Dottable,
    error::{OpmResult, OpossumError},
    lightdata::LightData,
    optic_ports::OpticPorts,
    optical::{LightResult, Optical},
    properties::{PropCondition, Properties, Proptype},
};
use ndarray::{array, Array1};
use uom::{si::f64::Length, si::length::meter};

pub struct IdealLens;

#[derive(Debug)]
pub struct RealLens {
    aperture: Length,
    curvatures: Array1<Length>,
    center_thickness: Length,
    z_pos: Length,
    refractive_index: f64,
    props: Properties,
}
fn create_default_props() -> Properties {
    let mut props = Properties::default();
    props
        .create(
            "name",
            "name of the lens",
            Some(vec![PropCondition::NonEmptyString]),
            "lens".into(),
        )
        .unwrap();
    props
        .create(
            "node_type",
            "specific optical type of this node",
            Some(vec![PropCondition::NonEmptyString]),
            "lens".into(),
        )
        .unwrap();
    props
        .create("inverted", "inverse propagation?", None, false.into())
        .unwrap();
    props
}

impl RealLens {
    pub fn new(
        aperture: Length,
        front_curvature: Length,
        rear_curvature: Length,
        center_thickness: Length,
        z_pos: Length,
        refractive_index: f64,
    ) -> Self {
        Self {
            aperture,
            curvatures: array![front_curvature, rear_curvature],
            center_thickness,
            z_pos,
            refractive_index,
            props: create_default_props(),
        }
    }

    pub fn get_aperture(&self) -> Length {
        self.aperture
    }

    pub fn set_aperture(&mut self, aperture: f64) {
        self.aperture = Length::new::<meter>(aperture);
    }

    pub fn get_curvatures(&self) -> &Array1<Length> {
        &self.curvatures
    }

    pub fn set_curvatures(&mut self, curvature_1: f64, curvature_2: f64) {
        self.curvatures = array![
            Length::new::<meter>(curvature_1),
            Length::new::<meter>(curvature_2)
        ];
    }

    pub fn get_thickness(&self) -> Length {
        self.center_thickness
    }

    pub fn set_thickness(&mut self, thickness: f64) {
        self.center_thickness = Length::new::<meter>(thickness);
    }

    pub fn get_position(&self) -> Length {
        self.z_pos
    }

    pub fn set_position(&mut self, position: f64) {
        self.z_pos = Length::new::<meter>(position);
    }

    pub fn get_refractve_index(&self) -> f64 {
        self.refractive_index
    }

    pub fn set_refractve_index(&mut self, refractive_index: f64) {
        self.refractive_index = refractive_index;
    }

    fn analyze_ray_trace(&mut self, incoming_data: LightResult) -> OpmResult<LightResult> {
        let _in1: Option<&Option<LightData>> = incoming_data.get("in1");
        Ok(incoming_data)

        // let mut in_rays: Vec<RayDataParaxial> = Vec::new();
        // if let Some(Some(in1)) = in1 {
        //     match in1 {
        //         LightData::ParAxialRayTrace(rays) => in_rays = rays.rays,
        //         _ => return Err(OpossumError::Analysis("expected set of rays".into())),
        //     }
        // };

        // let out1_energy = Some(LightData::Energy(DataEnergy {
        //     energy: in1_energy * self.ratio + in2_energy * (1.0 - self.ratio),
        // }));
        // Ok(HashMap::from([
        //     ("out1_trans1_refl2".into(), out1_energy),
        //     ("out2_trans2_refl1".into(), out2_energy),
        // ]))
    }

    // fn ray_propagate(&mut self, rays: &mut Vec<RayDataParaxial>){
    //     for ray in rays.into_iter(){
    //         if ray.pos
    //     };
    // }

    // pub struct RayDataParaxial {
    //     // ray: Array1<f64>,
    //     ray: Array1<f64>,
    //     pos: Vec<[f64;3]>,
    //     index: usize,
    //     bounce_lvl: usize,
    //     max_bounces: usize,
    // }
}

impl Default for RealLens {
    /// Create a 100mm focal lengths lens. LA1251-B from thorlabs. refractive inde hardcoded for n-bk7 at 1054 nm
    fn default() -> Self {
        Self {
            aperture: Length::new::<meter>(25e-3),
            curvatures: array![
                Length::new::<meter>(51.5e-3),
                Length::new::<meter>(f64::INFINITY)
            ],
            center_thickness: Length::new::<meter>(3.6e-3),
            z_pos: Length::new::<meter>(0.0),
            refractive_index: 1.5068,
            props: create_default_props(),
        }
    }
}

impl Optical for RealLens {
    fn ports(&self) -> OpticPorts {
        let mut ports = OpticPorts::new();
        ports.add_input("in1").unwrap();
        ports.add_output("out1").unwrap();
        ports
    }

    fn analyze(
        &mut self,
        incoming_data: LightResult,
        analyzer_type: &AnalyzerType,
    ) -> OpmResult<LightResult> {
        match analyzer_type {
            AnalyzerType::Energy => Err(OpossumError::Analysis(
                "Energy Analysis is not yet implemented for Lens Nodes".into(),
            )),
            AnalyzerType::ParAxialRayTrace => self.analyze_ray_trace(incoming_data),
        }
    }
    fn properties(&self) -> &Properties {
        &self.props
    }
    fn set_property(&mut self, name: &str, prop: Proptype) -> OpmResult<()> {
        self.props.set(name, prop)
    }
}

impl Dottable for RealLens {
    fn node_color(&self) -> &str {
        "blue"
    }
}
