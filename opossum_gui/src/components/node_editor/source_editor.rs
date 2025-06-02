use std::collections::HashMap;

use dioxus::prelude::*;
use opossum_backend::{energy_data_builder::EnergyDataBuilder, joule, light_data_builder::{self, LightDataBuilder}, millimeter, nanometer, ray_data_builder::{self, RayDataBuilder}, Grid, HexagonalTiling, Hexapolar, LaserLines, NodeAttr, PosDistType, Proptype, Random, UniformDist};

use super::node_editor_component::NodeChange;

struct SourceSelection{
    rays: bool,
    energy: bool
}
impl SourceSelection{
    pub fn new() -> Self{
        Self { rays: true, energy: false}
    }

    pub fn set_to_rays(&mut self){
        self.rays = true;
        self.energy = false;
    }

    pub fn set_to_energy(&mut self){
        self.rays = false;
        self.energy = true;
    }

    pub fn rays(&self) -> bool{
        self.rays
    }
    pub fn energy(&self) -> bool{
        self.energy
    }
}

#[derive(Clone, PartialEq)]
struct PosDistSelection{
    pub pos_dist: PosDistType,
    pub rand: bool,
    pub grid: bool,
    pub hexagonal: bool,
    pub hexapolar: bool,
    pub fibonacci_rect: bool,
    pub fibonacci_ell: bool,
    pub sobol: bool,    
}

impl PosDistSelection{
    pub fn new (pos_dist: PosDistType) -> Self{
        let mut select = Self { pos_dist: pos_dist.clone(), rand: false, grid: false, hexagonal: false, hexapolar: false, fibonacci_rect: false, fibonacci_ell: false, sobol: false};

        select.set_dist(pos_dist);
        select
    }
    pub fn set_dist(&mut self, pos_dist: PosDistType){
        (self.rand, self.grid, self.hexagonal, self.hexapolar, self.fibonacci_rect, self. fibonacci_rect, self.sobol) = match pos_dist{
            PosDistType::Random(_) => (true, false, false,false,false,false,false),
            PosDistType::Grid(_) => (false, true, false,false,false,false,false),
            PosDistType::HexagonalTiling(_) => (false, false, true,false,false,false,false),
            PosDistType::Hexapolar(_) => (false, false, false,true,false,false,false),
            PosDistType::FibonacciRectangle(_) => (false, false, false,false,true,false,false),
            PosDistType::FibonacciEllipse(_) => (false, false, false,false,false,true,false),
            PosDistType::Sobol(_) => (false, false, false,false,false,false,true),
        };

        self.pos_dist = pos_dist;
    }
}


struct RayTypeSelection{
    pub ray_type: RayDataBuilder,
    pub collimated: bool,
    pub point_src: bool,
    pub raw: bool,
    pub image: bool,
}

impl RayTypeSelection{
    pub fn new (ray_type: RayDataBuilder) -> Self{
        let mut select = Self { ray_type: ray_type.clone(), collimated: false, point_src: false, raw: false, image: false};

        select.set_ray_type(ray_type);
        select
    }
    pub fn set_ray_type(&mut self, ray_type: RayDataBuilder){
        (self.collimated, self.point_src, self.raw, self.image) = match ray_type{
            RayDataBuilder::Collimated {.. } => (true, false, false, false),
            RayDataBuilder::PointSrc {.. } => (false, true, false, false),
            RayDataBuilder::Raw(_) => (false, false, true, false),
            RayDataBuilder::Image {..} => (false, false, false, true),
        };

        self.ray_type = ray_type;
    }
}

// pub fn define_init_light_source_parameters(){
//     let random = Random::new(millimeter!(5.), millimeter!(5.),1000).unwrap();
//     let hexapolar =Hexapolar::new(millimeter!(5.), 5).unwrap();
//     let hexagonal =HexagonalTiling::new(millimeter!(5.), 5, millimeter!(0.,0.)).unwrap();
//     let hexagonal =Grid::new(millimeter!(5., 5.), (100,100)).unwrap();



//     let geom_light_data = LightDataBuilder::Geometric(RayDataBuilder::Collimated {
//     pos_dist: Hexapolar::new(millimeter!(5.), 5).unwrap().into(),
//     energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
//     spect_dist: LaserLines::new(vec![(nanometer!(1054.0), 1.0)])
//         .unwrap()
//         .into(),
//     });
//     let energy_light_data = LightDataBuilder::Energy(EnergyDataBuilder::LaserLines(
//         vec![(nanometer!(1054.0), joule!(1.0))],
//         nanometer!(1.0),
//     ));
//     let mut light_data_builder_hist = HashMap::<String, LightDataBuilder>::new();
//     light_data_builder_hist.insert("Rays".to_string(), geom_light_data);
//     light_data_builder_hist.insert("Energy".to_string(), energy_light_data);

// }

#[component]
pub fn SourceEditor(hide: bool, light_data_builder: LightDataBuilder, node_change: Signal<Option<NodeChange>>) -> Element{
    let pos_dist= Hexapolar::new(millimeter!(5.), 5).unwrap();
    let energy_dist= UniformDist::new(joule!(1.)).unwrap();
    let spect_dist= LaserLines::new(vec![(nanometer!(1054.0), 1.0)]).unwrap();
    let src_selection = use_signal(|| SourceSelection::new());
    let pos_dist_selection = use_signal(|| PosDistSelection::new(Hexapolar::default().into()));
    let ray_type_selection = use_signal(|| RayTypeSelection::new(RayDataBuilder::default()));
    let energy_data_builder_sig = use_signal(|| {
        match &light_data_builder{
            LightDataBuilder::Energy(energy_data_builder) => Some(energy_data_builder.clone()),
            _ => None,
        }
    });
    
    let ray_data_builder_sig = use_signal(|| {
        match  &light_data_builder{
            LightDataBuilder::Geometric(ray_data_builder) => Some(ray_data_builder.clone()),
            _ => None,
        }
    });
    

    let mut light_data_builder_hist = HashMap::<String, LightDataBuilder>::new();
    light_data_builder_hist.insert("Rays".to_string(), LightDataBuilder::default());

    rsx!{
        div {
            hidden: hide,
            class: "accordion accordion-borderless bg-dark ",
            id: "accordionSource",
            div { class: "accordion-item bg-dark text-light",
                h2 { class: "accordion-header", id: "sourceHeading",
                    button {
                        class: "accordion-button collapsed bg-dark text-light",
                        r#type: "button",
                        "data-mdb-collapse-init": "",
                        "data-mdb-target": "#sourceCollapse",
                        "aria-expanded": "false",
                        "aria-controls": "sourceCollapse",
                        "Light Source"
                    }
                }
                div {
                    id: "sourceCollapse",
                    class: "accordion-collapse collapse  bg-dark",
                    "aria-labelledby": "sourceHeading",
                    "data-mdb-parent": "#accordionSource",
                    div { class: "accordion-body  bg-dark",
                        SourceLightDataBuilderSelector{src_selection,light_data_builder, node_change, ray_data_builder_sig, light_data_builder_hist },
                        RayDataBuilderSelector{src_selection, ray_data_builder_sig: ray_type_selection, node_change },
                        RayPositionDistributionSelector{src_selection, rays_pos_dist_signal: pos_dist_selection, node_change} ,
                        RayDistributionEditor{src_selection, rays_pos_dist: pos_dist_selection, node_change},
                        // PosDistBuilderSelector{src_selection, ray_data_builder_sig, node_change }
                        
                    }
                }
            }
        }
    }
}

#[component]
pub fn SourceLightDataBuilderSelector(src_selection: Signal<SourceSelection>, light_data_builder: LightDataBuilder, node_change: Signal<Option<NodeChange>>, ray_data_builder_sig: Signal<Option<RayDataBuilder>>, light_data_builder_hist: HashMap<String, LightDataBuilder>) -> Element{
    rsx!{
        div { class:"form-floating",
            select {
                class: "form-select",
                id: "selectSourceType",
                "aria-label": "Select source type",
                onchange: {
                    move |e: Event<FormData>| {    
                        light_data_builder_hist.get(e.value().as_str()).cloned().map(|l|{
                            match l{
                                LightDataBuilder::Geometric(ray_data_builder) => {src_selection.write().set_to_rays(); ray_data_builder_sig.set(Some(ray_data_builder))},
                                _ => {src_selection.write().set_to_energy(); ray_data_builder_sig.set(None)},
                            }
                        });
                        // match e.value().as_str() {
                        //     "Rays" => {ray_data_builder_signal.set(light_data_builder_hist.get(e.value().as_str()).cloned())}
                        //     "Energy" => {ray_data_builder_signal.set(None)}
                        // };                    
                        node_change
                            .set(
                                Some(
                                    NodeChange::Property(
                                        "light data".to_owned(),
                                        serde_json::to_value(
                                                Proptype::LightDataBuilder(
                                                    light_data_builder_hist.get(e.value().as_str()).cloned(),
                                                ),
                                            )
                                            .unwrap(),
                                    ),
                                ),
                            );
                    }
                },
                {
                    match light_data_builder {
                        LightDataBuilder::Energy(_) => {
                            rsx! {
                                option { selected: true, value: "Energy", "Energy" }
                                option { value: "Rays", "Rays" }
                            }
                        }
                        LightDataBuilder::Geometric(_) => {
                            rsx! {
                                option { value: "Energy", "Energy" }
                                option { selected: true, value: "Rays", "Rays" }
                            }
                        }
                        _ => rsx! {
                            option { value: "Energy", "Energy" }
                            option { selected: true, value: "Rays", "Rays" }
                        },
                    }                   
                    
                }

            },
            label { r#for: "selectSourceType", "Source Type" }
            
        }
    }
}


#[component]
pub fn RayDataBuilderSelector(src_selection: Signal<SourceSelection>, ray_data_builder_sig: Signal<RayTypeSelection>, node_change: Signal<Option<NodeChange>>) -> Element{
    rsx!{
        div { class: "form-floating",
                hidden: !src_selection.read().rays(),
            select {
                class: "form-select",
                id: "selectRaySourceType",
                "aria-label": "Select ray source type",
                // onchange: {
                //     let light_data_builder_opt = light_data_builder_opt.clone();
                //     move |e: Event<FormData>| {
                //         node_change
                //             .set(
                //                 Some(
                //                     NodeChange::Property(
                //                         "light data".to_owned(),
                //                         serde_json::to_value(
                //                                 Proptype::LightDataBuilder(
                //                                     light_data_builder_opt.clone(),
                //                                 ),
                //                             )
                //                             .unwrap(),
                //                     ),
                //                 ),
                //             );
                //     }
                // },
                option { selected: ray_data_builder_sig.read().collimated, value: "Collimated", "Collimated" },
                option { selected: ray_data_builder_sig.read().point_src, value: "Point Source", "Point Source" }
                }
                label { r#for: "selectRaySourceType", "Rays Type" },
        }  
    }
}

#[component]
pub fn RayDataBuilderDistributions(hide: bool, ray_data_builder_sig: Signal<Option<RayDataBuilder>>, node_change: Signal<Option<NodeChange>>) -> Element{
    let ray_data_builder_opt = ray_data_builder_sig.read().clone();
    rsx!{
        div { class: "form-floating",
                hidden: ray_data_builder_opt.is_none(),
            select {
                class: "form-select",
                id: "selectRaysDistribution",
                "aria-label": "Select ray source type",
                {
                    if let Some(ref ray_data_builder) = ray_data_builder_opt{
                        match ray_data_builder {
                            RayDataBuilder::Collimated (collimated_src) => 
                            rsx! {
                                    option { selected: true, value: "Collimated", "Collimated" }
                                    option { value: "Point Source", "Point Source" }
                                },
                            RayDataBuilder::PointSrc(point_src) => 
                            rsx! {
                                    p{"hier weiter machen"}
                                },
                                _  => rsx!{}
                        }
                    }
                    else{
                     rsx!{}
                    }
                }

                },
            
            label { r#for: "selectRaysDistribution", "Rays Distributions" }
        }
    }
}

#[component]
pub fn RayPositionDistributionSelector(src_selection: Signal<SourceSelection>, rays_pos_dist_signal: Signal<PosDistSelection>, node_change: Signal<Option<NodeChange>>) -> Element{

    rsx!{
        div { class: "form-floating",
                hidden: !src_selection.read().rays(),
            select {
                class: "form-select",
                id: "selectRaysPosDistribution",
                // onchange : move |e: Event<FormData>| {
                //     match e.value().as_str(){
                //         "Random" => rays_pos_dist_signal.write().set_dist(PosDistType::Random(Random::new(millimeter!(1.), millimeter!(1.), 1000))),
                //         "Grid" => rays_pos_dist_signal.write().grid = true,
                //         "Hexagonal" => rays_pos_dist_signal.write().hexagonal = true,
                //         "Hexapolar" => rays_pos_dist_signal.write().hexapolar = true,
                //         "Fibonacci, rectangular" => rays_pos_dist_signal.write().fibonacci_rect = true,
                //         "Fibonacci, elliptical" => rays_pos_dist_signal.write().rand = true,
                //         "Sobol" => todo!(),
                //         _ => todo!(),
                //     }
                // },
                "aria-label": "Select ray position distribution",
                    option { selected: rays_pos_dist_signal.read().rand, value:     "Random", "Random" }
                    option { selected: rays_pos_dist_signal.read().grid, value:     "Grid", "Grid" }
                    option { selected: rays_pos_dist_signal.read().hexagonal, value:  "Hexagonal", "Hexagonal" }
                    option { selected: rays_pos_dist_signal.read().hexapolar, value:  "Hexapolar", "Hexapolar" }
                    option { selected: rays_pos_dist_signal.read().fibonacci_rect, value:  "Fibonacci, rectangular", "Fibonacci, rectangular" }
                    option { selected: rays_pos_dist_signal.read().fibonacci_ell, value:   "Fibonacci, elliptical", "Fibonacci, elliptical" }
                    option { selected: rays_pos_dist_signal.read().sobol, value:    "Sobol", "Sobol" }
                },
            label { r#for: "selectRaysPosDistribution", "Rays Position Distribution" }
        }
    }    
}

#[component]
pub fn RayDistributionEditor(src_selection: Signal<SourceSelection>, rays_pos_dist: Signal<PosDistSelection>, node_change: Signal<Option<NodeChange>>) -> Element{

    rsx!{
        div {
            hidden: !src_selection.read().rays(),

                div { class: "form-floating", "data-mdb-input-init": "",
                    input {
                        class: "form-control bg-dark text-light",
                        r#type: "number",
                        id: "hexapolarRings",
                        name: "hexapolarRings",
                        placeholder: "Number of rings",
                        value: "3",
                        "readonly": false,
                    }
                    label { class: "form-label", r#for: "hexapolarRings", "Number of rings"} },
                
                div { class: "form-floating", "data-mdb-input-init": "",
                    input {
                        class: "form-control bg-dark text-light",
                        r#type: "number",
                        id: "hexapolarRadius",
                        name: "hexapolarRadius",
                        placeholder: "Radius in mm",
                        value: "3",
                        "readonly": false,
                    }
                    label { class: "form-label", r#for: "hexapolarRadius", "Radius in mm"} 
                }
            }
        }
    }
                            



// RayDataBuilderParams{ray_data_builder_sig, node_change }
                           
                        
                    //     {
                    //         let node_props = node_attr.properties().clone();
                    //         if let Ok(light_data) = node_props.get("light data") {
                    //             rsx! {
                    //                 {
                    //                     {
                    //                     if let Proptype::LightDataBuilder(Some(light_data_builder)) = light_data{
                    //                         rsx!{
                    //                             SourceLightDataBuilderSelector{light_data_builder: light_data_builder.clone(), node_change: node_change.clone() }
                    //                         }
                    //                     }
                    //                     else{
                    //                         rsx!{}
                    //                     }
                    //                     if let Proptype::LightDataBuilder(
                    //                         Some(LightDataBuilder::Geometric(ray_data_builder)),
                    //                     ) = light_data {
                    //                         rsx! {
                    //                             div { class: "form-floating", id: "selectRayType",
                    //                                 select {
                    //                                     class: "form-select",
                    //                                     "aria-label": "Select rays type",
                    //                                     onchange: {
                    //                                         let mut light_data_builder = light_data_builder.clone();
                    //                                         let mut ray_data_builder = ray_data_builder.clone();
                    //                                         move |e: Event<FormData>| {
                    //                                             match e.value().as_str() {
                    //                                                 "Collimated" => {
                    //                                                     light_data_builder
                    //                                                         .insert(
                    //                                                             "Rays".to_string(),
                    //                                                             LightDataBuilder::Geometric(RayDataBuilder::Collimated {
                    //                                                                 pos_dist: Hexapolar::new(millimeter!(5.), 5)
                    //                                                                     .unwrap()
                    //                                                                     .into(),
                    //                                                                 energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
                    //                                                                 spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])
                    //                                                                     .unwrap()
                    //                                                                     .into(),
                    //                                                             }),
                    //                                                         );
                    //                                                 }
                    //                                                 "Point Source" => {
                    //                                                     light_data_builder
                    //                                                         .insert(
                    //                                                             "Rays".to_string(),
                    //                                                             LightDataBuilder::Geometric(RayDataBuilder::PointSrc {
                    //                                                                 pos_dist: Hexapolar::new(millimeter!(5.), 5)
                    //                                                                     .unwrap()
                    //                                                                     .into(),
                    //                                                                 energy_dist: UniformDist::new(joule!(1.)).unwrap().into(),
                    //                                                                 spect_dist: LaserLines::new(vec![(nanometer!(1000.0), 1.0)])
                    //                                                                     .unwrap()
                    //                                                                     .into(),
                    //                                                                 reference_length: millimeter!(1000.),
                    //                                                             }),
                    //                                                         );
                    //                                                 }
                    //                                                 _ => {}
                    //                                             }
                    //                                             node_change
                    //                                                 .set(
                    //                                                     Some(
                    //                                                         NodeChange::Property(
                    //                                                             "light data".to_owned(),
                    //                                                             serde_json::to_value(
                    //                                                                     Proptype::LightDataBuilder(
                    //                                                                         light_data_builder.get("Rays").cloned(),
                    //                                                                     ),
                    //                                                                 )
                    //                                                                 .unwrap(),
                    //                                                         ),
                    //                                                     ),
                    //                                                 );
                    //                                         }
                    //                                     },
                    //                                     {
                    //                                         match ray_data_builder {
                    //                                             RayDataBuilder::Collimated { .. } => rsx! {
                    //                                                 option { selected: true, value: "Collimated", "Collimated" }
                    //                                                 option { value: "Point Source", "Point Source" }
                    //                                             },
                    //                                             RayDataBuilder::PointSrc { .. } => rsx! {
                    //                                                 option { value: "Collimated", "Collimated" }
                    //                                                 option { selected: true, value: "Point Source", "Point Source" }
                    //                                             },
                    //                                             _ => rsx! {},
                    //                                         }
                    //                                     }
                    //                                 }
                    //                                 label { r#for: "selectRayType", "Source Type" }
                    //                             }
                    //                         }
                    //                     } else {
                    //                         rsx! {}
                    //                     }
                    //                 }
                    //             }
                    //         } else {
                    //             rsx! {
                    //                 option { selected: true, disabled: true, value: "None", "None" }
                    //                 option { value: "Energy", "Energy" }
                    //                 option { value: "Rays", "Rays" }
                    //             }
                    //         }
                    //     }