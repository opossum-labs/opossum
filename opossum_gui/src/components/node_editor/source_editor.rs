use std::{collections::HashMap, fmt::Display};

use dioxus::prelude::*;
use nalgebra::Point;
use opossum_backend::{
    energy_data_builder::EnergyDataBuilder, joule, light_data_builder::{self, LightDataBuilder}, millimeter, nanometer, ray_data_builder::{self, CollimatedSrc, PointSrc, RayDataBuilder}, FibonacciEllipse, FibonacciRectangle, Grid, HexagonalTiling, Hexapolar, Isometry, LaserLines, NodeAttr, PosDistType, Proptype, Random, SobolDist, UniformDist
};
use uom::si::length::millimeter;

use crate::{components::node_editor::accordion::AccordionItem, OPOSSUM_UI_LOGS};

use super::node_editor_component::NodeChange;

struct SourceSelection {
    rays: bool,
    energy: bool,
}
impl SourceSelection {
    pub fn new() -> Self {
        Self {
            rays: true,
            energy: false,
        }
    }

    pub fn set_to_rays(&mut self) {
        self.rays = true;
        self.energy = false;
    }

    pub fn set_to_energy(&mut self) {
        self.rays = false;
        self.energy = true;
    }

    pub fn rays(&self) -> bool {
        self.rays
    }
    pub fn energy(&self) -> bool {
        self.energy
    }
}

#[derive(Clone, PartialEq)]
struct PosDistSelection {
    pub pos_dist: PosDistType,
    pub rand: bool,
    pub grid: bool,
    pub hexagonal: bool,
    pub hexapolar: bool,
    pub fibonacci_rect: bool,
    pub fibonacci_ell: bool,
    pub sobol: bool,
}

impl PosDistSelection {
    pub fn new(pos_dist: PosDistType) -> Self {
        let mut select = Self {
            pos_dist: pos_dist.clone(),
            rand: false,
            grid: false,
            hexagonal: false,
            hexapolar: false,
            fibonacci_rect: false,
            fibonacci_ell: false,
            sobol: false,
        };

        select.set_dist(pos_dist);
        select
    }
    pub fn set_dist(&mut self, pos_dist: PosDistType) {
        (
            self.rand,
            self.grid,
            self.hexagonal,
            self.hexapolar,
            self.fibonacci_rect,
            self.fibonacci_rect,
            self.sobol,
        ) = match pos_dist {
            PosDistType::Random(_) => (true, false, false, false, false, false, false),
            PosDistType::Grid(_) => (false, true, false, false, false, false, false),
            PosDistType::HexagonalTiling(_) => (false, false, true, false, false, false, false),
            PosDistType::Hexapolar(_) => (false, false, false, true, false, false, false),
            PosDistType::FibonacciRectangle(_) => (false, false, false, false, true, false, false),
            PosDistType::FibonacciEllipse(_) => (false, false, false, false, false, true, false),
            PosDistType::Sobol(_) => (false, false, false, false, false, false, true),
        };

        self.pos_dist = pos_dist;
    }
}

impl TryFrom<LightDataBuilder> for PosDistSelection {
    type Error = String;

    fn try_from(value: LightDataBuilder) -> Result<Self, Self::Error> {
        match value {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Ok(Self::new(collimated_src.pos_dist().clone()))
                }
                RayDataBuilder::PointSrc(point_src) => Ok(Self::new(point_src.pos_dist().clone())),
                RayDataBuilder::Raw(rays) => Err("not used yet".to_owned()),
                RayDataBuilder::Image {
                    file_path,
                    pixel_size,
                    total_energy,
                    wave_length,
                    cone_angle,
                } => Err("not used yet".to_owned()),
            },
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}

struct RayTypeSelection {
    pub ray_type: RayDataBuilder,
    pub collimated: bool,
    pub point_src: bool,
    pub raw: bool,
    pub image: bool,
}

impl RayTypeSelection {
    pub fn new(ray_type: RayDataBuilder) -> Self {
        let mut select = Self {
            ray_type: ray_type.clone(),
            collimated: false,
            point_src: false,
            raw: false,
            image: false,
        };

        select.set_ray_type(ray_type);
        select
    }
    pub fn set_ray_type(&mut self, ray_type: RayDataBuilder) {
        (self.collimated, self.point_src, self.raw, self.image) = match ray_type {
            RayDataBuilder::Collimated { .. } => (true, false, false, false),
            RayDataBuilder::PointSrc { .. } => (false, true, false, false),
            RayDataBuilder::Raw(_) => (false, false, true, false),
            RayDataBuilder::Image { .. } => (false, false, false, true),
        };

        self.ray_type = ray_type;
    }
}

impl TryFrom<LightDataBuilder> for RayTypeSelection {
    type Error = String;

    fn try_from(value: LightDataBuilder) -> Result<Self, Self::Error> {
        match value {
            LightDataBuilder::Geometric(ray_data_builder) => Ok(Self::new(ray_data_builder)),
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct LightDataBuilderHistory {
    hist: HashMap<String, LightDataBuilder>,
    current: String,
}
impl LightDataBuilderHistory {
    pub fn default() -> Self {
        let current = "Rays".to_owned();
        let ld_builder = LightDataBuilder::Geometric(RayDataBuilder::default());
        let mut hist = HashMap::<String, LightDataBuilder>::new();
        hist.insert(current.clone(), ld_builder);
        Self { hist, current }
    }
    pub fn get_current(&self) -> &LightDataBuilder {
        self.hist.get(&self.current).unwrap()
    }

    pub fn get_current_mut(&mut self) -> &mut LightDataBuilder {
        self.hist.get_mut(&self.current).unwrap()
    }

    pub fn get_current_key(&self) -> &str {
        self.current.as_str()
    }

    pub fn set_current(&mut self, key: &str) -> bool {
        if let Some(_) = self.hist.get(key) {
            self.current = key.to_owned();
            true
        } else {
            false
        }
    }

    pub fn get(&self, key: &str) -> Option<&LightDataBuilder> {
        self.hist.get(key)
    }
    pub fn insert_and_set_current(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
        self.current = key.to_owned();
    }

    pub fn insert(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
    }

    pub fn replace_or_insert(&mut self, key: &str, new_ld_builder: LightDataBuilder) {
        if let Some(ld_builder) = self.hist.get_mut(key) {
            *ld_builder = new_ld_builder;
        } else {
            self.insert(key, new_ld_builder);
        }
    }

    pub fn replace_or_insert_and_set_current(
        &mut self,
        key: &str,
        new_ld_builder: LightDataBuilder,
    ) {
        if let Some(ld_builder) = self.hist.get_mut(key) {
            *ld_builder = new_ld_builder;
        } else {
            self.insert(key, new_ld_builder);
        }
        self.current = key.to_owned();
    }

    pub fn is_rays_is_collimated(&self) -> (bool, bool) {
        match self.get_current() {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(_) => (true, true),
                _ => (true, false),
            },
            _ => (false, false),
        }
    }

    pub fn get_current_ray_data_builder(&self) -> Option<RayDataBuilder> {
        match self.get_current() {
            LightDataBuilder::Geometric(ray_data_builder) => Some(ray_data_builder.clone()),
            _ => None,
        }
    }

    pub fn get_current_pos_dist_type(&self) -> Option<PosDistType> {
        match self.get_current() {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Some(collimated_src.pos_dist().clone())
                }
                RayDataBuilder::PointSrc(point_src) => Some(point_src.pos_dist().clone()),
                RayDataBuilder::Raw(rays) => None,
                RayDataBuilder::Image {
                    file_path,
                    pixel_size,
                    total_energy,
                    wave_length,
                    cone_angle,
                } => None,
            },
            _ => None,
        }
    }

    pub fn set_pos_dist_type(&mut self, new_pos_dist: PosDistType) {
        if let Some(rdb) = &mut self.get_current_ray_data_builder() {
            let pos_dist_string = format!("{new_pos_dist}");
            let new_ld_builder = match rdb {
                RayDataBuilder::Raw(rays) => todo!(),
                RayDataBuilder::Collimated(collimated_src) => {
                    collimated_src.set_pos_dist(new_pos_dist);
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        collimated_src.clone(),
                    ));
                    self.replace_or_insert("Collimated", new_ld_builder.clone());
                    new_ld_builder
                }
                RayDataBuilder::PointSrc(point_src) => {
                    point_src.set_pos_dist(new_pos_dist);
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src.clone()));
                    self.replace_or_insert("Point Source", new_ld_builder.clone());
                    new_ld_builder
                }
                RayDataBuilder::Image {
                    file_path,
                    pixel_size,
                    total_energy,
                    wave_length,
                    cone_angle,
                } => todo!(),
            };
            self.replace_or_insert("Rays", new_ld_builder.clone());
            self.replace_or_insert_and_set_current(&pos_dist_string, new_ld_builder);
        }
    }
}

#[component]
pub fn SourceEditor(
    hide: bool,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    // let mut light_data_builder_sig = Signal::new(light_data_builder);

    use_memo(move || {
        node_change.set(Some(NodeChange::Property(
            "light data".to_owned(),
            serde_json::to_value(Proptype::LightDataBuilder(Some(
                light_data_builder_sig.read().get_current().clone(),
            )))
            .unwrap(),
        )))
    });

    use_effect(move || {
        node_change.set(Some(NodeChange::Isometry(Isometry::identity())))
    });

    let (is_rays, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let accordion_item_content = rsx!{
        SourceLightDataBuilderSelector { light_data_builder_sig }
            RayDataBuilderSelector { light_data_builder_sig }
            ReferenceLengthEditor { light_data_builder_sig }
            
            div {
                hidden: !is_rays,
                class: "accordion accordion-borderless bg-dark border-start",
                id: "accordionSourceDists",
                PositionDistributionEditor { light_data_builder_sig }
                // EnergyDistributionEditor { light_data_builder_sig }
                // SpectralDistributionEditor { light_data_builder_sig }
            }
    };


    rsx! {
        AccordionItem {elements: vec![accordion_item_content], header: "Light Source", id: "sourceHeading", parent: "accordionNodeConfig", content_id: "sourceCollapse"}
        // div { class: "accordion-item bg-dark text-light", hidden: hide,
        //     h2 { class: "accordion-header", id: "sourceHeading",
        //         button {
        //             class: "accordion-button collapsed bg-dark text-light",
        //             r#type: "button",
        //             "data-mdb-collapse-init": "",
        //             "data-mdb-target": "#sourceCollapse",
        //             "aria-expanded": "false",
        //             "aria-controls": "sourceCollapse",
        //             "Light Source"
        //         }
        //     }
        //     div {
        //         id: "sourceCollapse",
        //         class: "accordion-collapse collapse  bg-dark",
        //         "aria-labelledby": "sourceHeading",
        //         "data-mdb-parent": "#accordionNodeConfig",
        //         div { class: "accordion-body  bg-dark",
        //             SourceLightDataBuilderSelector { light_data_builder_sig }
        //             RayDataBuilderSelector { light_data_builder_sig }
        //             ReferenceLengthEditor { light_data_builder_sig }
                    
        //             div {
        //                 hidden: !is_rays,
        //                 class: "accordion accordion-borderless bg-dark border-start",
        //                 id: "accordionSourceDists",
        //                 PositionDistributionEditor { light_data_builder_sig }
        //                 // EnergyDistributionEditor { light_data_builder_sig }
        //                 // SpectralDistributionEditor { light_data_builder_sig }
        //             }
        //         }
        //     }
        // }
    }
    // }
}

#[component]
pub fn ReferenceLengthEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>,) -> Element{
    let (is_rays, is_collimated) = light_data_builder_sig.read().is_rays_is_collimated();
    if let Some(RayDataBuilder::PointSrc(point_src)) =  light_data_builder_sig.read().get_current_ray_data_builder(){

            rsx!{
            
                div { class: "form-floating border-start", "data-mdb-input-init": "", hidden: is_collimated,
                    input {
                        class: "form-control bg-dark text-light form-control-sm",
                        r#type: "number",
                        min: "0.0000000001",
                        id: "pointsrcRefLength",
                        name: "pointsrcRefLength",
                        placeholder: "Reference length in mm",
                        value: format!("{}", point_src.reference_length().get::<millimeter>()),
                        "readonly": false,
                        onchange: {
                            let point_src = point_src.clone();
                            move |e: Event<FormData>| {
                                let mut point_src = point_src.clone();
                                if let Ok(ref_length) = e.data.parsed::<f64>() {
                                    point_src.set_reference_length(millimeter!(ref_length));
                                    light_data_builder_sig
                                        .with_mut(|ldb| {
                                            if let LightDataBuilder::Geometric(RayDataBuilder::PointSrc(p)) = ldb.get_current_mut(){
                                                *p = point_src; 
                                            }
                                        });
                                }
                            }
                        },
                    }
                    label { class: "form-label text-secondary", r#for: "pointsrcRefLength", "Reference length in mm" }
                }
            }
        
    }
    else{
        rsx!{

        }
    }
}

#[component]
pub fn PositionDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: "sourcePositionDistHeading",
                button {
                    class: "accordion-button collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": "#sourcePositionDistCollapse",
                    "aria-expanded": "false",
                    "aria-controls": "sourcePositionDistCollapse",
                    "Position Distribution"
                }
            }
            div {
                id: "sourcePositionDistCollapse",
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": "sourcePositionDistHeading",
                "data-mdb-parent": "#accordionSourceDists",
                div { class: "accordion-body  bg-dark",
                    
                    RayPositionDistributionSelector { light_data_builder_sig }
                    RayDistributionEditor { light_data_builder_sig }
                }
            }
        }
    }
}

#[component]
pub fn EnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: "sourceEnergyDistHeading",
                button {
                    class: "accordion-button collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": "#sourceEnergyDistCollapse",
                    "aria-expanded": "false",
                    "aria-controls": "sourceEnergyDistCollapse",
                    "Energy Distribution"
                }
            }
            div {
                id: "sourceEnergyDistCollapse",
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": "sourceEnergyDistHeading",
                "data-mdb-parent": "#accordionSourceDists",
                div { class: "accordion-body  bg-dark" }
            }
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    rsx! {
        div { class: "accordion-item bg-dark text-light",
            h6 { class: "accordion-header", id: "sourceSpectralDistHeading",
                button {
                    class: "accordion-button collapsed bg-dark text-light",
                    r#type: "button",
                    "data-mdb-collapse-init": "",
                    "data-mdb-target": "#sourceSpectralDistCollapse",
                    "aria-expanded": "false",
                    "aria-controls": "sourceSpectralDistCollapse",
                    "Spectral Distribution"
                }
            }
            div {
                id: "sourceSpectralDistCollapse",
                class: "accordion-collapse collapse  bg-dark",
                "aria-labelledby": "sourceSpectralDistHeading",
                "data-mdb-parent": "#accordionSourceDists",
                div { class: "accordion-body  bg-dark" }
            }
        }
    }
}

#[component]
pub fn SourceLightDataBuilderSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (is_rays, _) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        div { class: "form-floating border-start",
            select {
                class: "form-select bg-dark text-light",
                id: "selectSourceType",
                "aria-label": "Select source type",
                onchange: {
                    move |e: Event<FormData>| {
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                let value = e.value();
                                if !ldb.set_current(value.as_str()) {
                                    match value.as_str() {
                                        "Rays" => {
                                            let new_ld_builder = LightDataBuilder::Geometric(
                                                RayDataBuilder::Collimated(CollimatedSrc::default()),
                                            );
                                            ldb.replace_or_insert_and_set_current(
                                                value.as_str(),
                                                new_ld_builder,
                                            );
                                        }
                                        "Energy" => {
                                            let new_ld_builder = LightDataBuilder::Energy(
                                                EnergyDataBuilder::default(),
                                            );
                                            ldb.replace_or_insert_and_set_current(
                                                value.as_str(),
                                                new_ld_builder.clone(),
                                            );
                                        }
                                        _ => {
                                            OPOSSUM_UI_LOGS
                                                .write()
                                                .add_log(
                                                    &format!("Unknown source type: {}", value.as_str()),
                                                )
                                        }
                                    };
                                }
                            });
                    }
                },
                {
                    rsx! {
                        option { selected: !is_rays, value: "Energy", "Energy" }
                        option { selected: is_rays, value: "Rays", "Rays" }
                    }
                }
            
            }
            label { class: "text-secondary", r#for: "selectSourceType", "Source Type" }
        }
    }
}

#[component]
pub fn RayDataBuilderSelector(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (show, is_collimated) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        div { class: "form-floating border-start", hidden: !show,
            select {
                class: "form-select bg-dark text-light",
                id: "selectRaySourceType",
                "aria-label": "Select ray source type",
                onchange: {
                    move |e: Event<FormData>| {
                        let value = e.value();
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                let val_str = value.as_str();
                                let new_ld_builder = if !ldb.set_current(val_str) {
                                    match value.as_str() {
                                        "Collimated" => {
                                            LightDataBuilder::Geometric(
                                                RayDataBuilder::Collimated(CollimatedSrc::default()),
                                            )
                                        }
                                        "Point Source" => {
                                            LightDataBuilder::Geometric(
                                                RayDataBuilder::PointSrc(PointSrc::default()),
                                            )
                                        }
                                        _ => {
                                            OPOSSUM_UI_LOGS
                                                .write()
                                                .add_log(&format!("Unknown ray source type: {}", val_str));
                                            LightDataBuilder::Geometric(RayDataBuilder::default())
                                        }
                                    }
                                } else {
                                    ldb.get_current().clone()
                                };
                                ldb.replace_or_insert("Rays", new_ld_builder.clone());
                                ldb.replace_or_insert_and_set_current(val_str, new_ld_builder);
                            });
                    }
                },
                option { selected: is_collimated, value: "Collimated", "Collimated" }
                option { selected: !is_collimated, value: "Point Source", "Point Source" }
            }
            label { class: "text-secondary", r#for: "selectRaySourceType", "Rays Type" }
        }
    }
}

#[component]
pub fn RayPositionDistributionSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist =
        PosDistSelection::try_from(light_data_builder_sig.read().get_current().clone());

    if let Ok(rpd) = rays_pos_dist {
        rsx! {
            div { class: "form-floating border-start", hidden: !show,
                select {
                    class: "form-select bg-dark text-light",
                    id: "selectRaysPosDistribution",
                    onchange: move |e: Event<FormData>| {
                        let value = e.value();
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                let mut ray_data_builder = ldb.get_current_ray_data_builder();
                                let val_str = value.as_str();
                                let new_ld_builder = if !ldb.set_current(val_str) {
                                    let pos_dist_type: PosDistType = match val_str {
                                        "Random" => Random::default().into(),
                                        "Grid" => Grid::default().into(),
                                        "Hexagonal" => HexagonalTiling::default().into(),
                                        "Hexapolar" => Hexapolar::default().into(),
                                        "Fibonacci, rectangular" => FibonacciRectangle::default().into(),
                                        "Fibonacci, elliptical" => FibonacciEllipse::default().into(),
                                        "Sobol" => SobolDist::default().into(),
                                        _ => Hexapolar::default().into(),
                                    };
                                    if let Some(ref mut rdb) = ray_data_builder {
                                        match rdb {
                                            RayDataBuilder::Collimated(ref mut collimated_src) => {
                                                collimated_src.set_pos_dist(pos_dist_type);
                                                ldb.replace_or_insert(
                                                    "Collimated",
                                                    LightDataBuilder::Geometric(rdb.clone()).clone(),
                                                );
                                            }
                                            RayDataBuilder::PointSrc(ref mut point_src) => {
                                                point_src.set_pos_dist(pos_dist_type);
                                                ldb.replace_or_insert(
                                                    "Point Source",
                                                    LightDataBuilder::Geometric(rdb.clone()).clone(),
                                                );
                                            }
                                            RayDataBuilder::Raw(rays) => todo!(),
                                            RayDataBuilder::Image {
                                                file_path,
                                                pixel_size,
                                                total_energy,
                                                wave_length,
                                                cone_angle,
                                            } => todo!(),
                                        };
                                        LightDataBuilder::Geometric(rdb.clone())
                                    } else {
                                        LightDataBuilder::Geometric(RayDataBuilder::default())
                                    }
                                } else {
                                    ldb.get_current().clone()
                                };
                                ldb.replace_or_insert("Rays", new_ld_builder.clone());
                                ldb.replace_or_insert_and_set_current(val_str, new_ld_builder);
                            })
                    },
                    "aria-label": "Select ray position distribution",
                    option { selected: rpd.rand, value: "Random", "Random" }
                    option { selected: rpd.grid, value: "Grid", "Grid" }
                    option { selected: rpd.hexagonal, value: "Hexagonal", "Hexagonal" }
                    option { selected: rpd.hexapolar, value: "Hexapolar", "Hexapolar" }
                    option {
                        selected: rpd.fibonacci_rect,
                        value: "Fibonacci, rectangular",
                        "Fibonacci, rectangular"
                    }
                    option {
                        selected: rpd.fibonacci_ell,
                        value: "Fibonacci, elliptical",
                        "Fibonacci, elliptical"
                    }
                    option { selected: rpd.sobol, value: "Sobol", "Sobol" }
                }
                label {
                    class: "text-secondary",
                    r#for: "selectRaysPosDistribution",
                    "Rays Position Distribution"
                }
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn RayDistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (show, _) = light_data_builder_sig.read().is_rays_is_collimated();
    let rays_pos_dist = light_data_builder_sig
        .read()
        .get_current_pos_dist_type()
        .clone();

    rsx! {
        div { hidden: !show,
            {
                if let Some(pos_dist_type) = rays_pos_dist {
                    match pos_dist_type {
                        PosDistType::Random(_) =>{
                        rsx! {

                                NodePosDistInput{pos_dist_type, param: PosDistParam::PointsX { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}

                                    div { class: "row gy-1 gx-2",
                                        div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::LengthX{ min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                        }
                                        div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::LengthY { min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                        }
                                    }
                            }
                        },
                        PosDistType::Grid(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::PointsX { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::PointsY { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}
                                    }
                                }
                                    div { class: "row gy-1 gx-2",
                                        div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::LengthX{ min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                        }
                                        div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::LengthY { min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                        }
                                    }
                            }
                        },
                        PosDistType::HexagonalTiling(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::Rings {min:1, max: 255, step:1}, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::Radius {min:1e-9, max: 1e9, step:1.}, light_data_builder_sig}
                                    }
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::CenterX {min:-1e9, max: 1e9, step:1.}, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput{pos_dist_type, param: PosDistParam::CenterY {min:-1e9, max: 1e9, step:1.}, light_data_builder_sig}
                                    }
                                }
                            }
                        }
                        PosDistType::Hexapolar(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::Rings{min:1, max: 255, step:1}, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::Radius {min:1e-9, max: 1e9, step:1.}, light_data_builder_sig}
                                    }
                                }
                            }
                        }
                        PosDistType::FibonacciRectangle(_) => {
                        rsx! {

                                NodePosDistInput{pos_dist_type, param: PosDistParam::PointsX { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthX{ min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthY { min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                }
                            }
                        },
                        PosDistType::FibonacciEllipse(_)=> {
                        rsx! {

                                NodePosDistInput{pos_dist_type, param: PosDistParam::PointsX { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthX{ min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthY { min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                }
                            }
                        },
                        PosDistType::Sobol(sobol_dist) => {
                        rsx! {

                                NodePosDistInput{pos_dist_type, param: PosDistParam::PointsX { min: 1, max: 1000000000, step: 1 }, light_data_builder_sig}
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthX{ min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                    div { class: "col-sm",
                                    NodePosDistInput{pos_dist_type, param: PosDistParam::LengthY { min: 1e-9, max: 1e9, step: 1. }, light_data_builder_sig}
                                    }
                                }
                            }
                        },
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum PosDistParam{
    Rings{min: u8, max: u8, step:u8},
    Radius{min: f64, max: f64, step:f64},
    CenterX{min: f64, max: f64, step:f64},
    CenterY{min: f64, max: f64, step:f64},
    LengthX{min: f64, max: f64, step:f64},
    LengthY{min: f64, max: f64, step:f64},
    PointsX{min: usize, max: usize, step:usize},
    PointsY{min: usize, max: usize, step:usize},
}

impl Display for PosDistParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param = match self{
            PosDistParam::Rings { min, max, step } => "Rings",
            PosDistParam::Radius { min, max, step } => "Radius",
            PosDistParam::CenterX { min, max, step } => "CenterX",
            PosDistParam::CenterY { min, max, step } => "CenterY",
            PosDistParam::LengthX { min, max, step } => "LengthX",
            PosDistParam::LengthY { min, max, step } => "lengthY",
            PosDistParam::PointsX { min, max, step } => "PointsX",
            PosDistParam::PointsY { min, max, step } => "PointsY",
        };
        write!(f, "{param}")
    }
}



#[component]
pub fn NodePosDistInput(pos_dist_type: PosDistType, param: PosDistParam, light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element{
    let name = format!("{pos_dist_type}{param}");

    let (place_holder, val, valid, min, max, step) = match pos_dist_type{
        PosDistType::Random(random) => {
            match param{
                PosDistParam::LengthX { min, max, step } => ("x length in mm".to_string(), format!("{}",random.side_length_x().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::LengthY { min, max, step } => ("y length in mm".to_string(), format!("{}",random.side_length_y().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsX { min, max, step } => ("#Points".to_string(), format!("{}",random.nr_of_points()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }},
        PosDistType::Grid(grid) => {
            match param{
                PosDistParam::LengthX { min, max, step } => ("x length in mm".to_string(), format!("{}",grid.side_length().0.get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::LengthY { min, max, step } => ("y length in mm".to_string(), format!("{}",grid.side_length().1.get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsX { min, max, step } => ("#Points along x".to_string(), format!("{}",grid.nr_of_points().0), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsY { min, max, step } => ("#Points along x".to_string(), format!("{}",grid.nr_of_points().1), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }},
        PosDistType::HexagonalTiling(hexagonal_tiling) => {
            match param{
                PosDistParam::Rings { min, max, step } =>  ("Number of rings".to_string(), format!("{}",hexagonal_tiling.nr_of_hex_along_radius()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::Radius { min, max, step } => ("Radius in mm".to_string(), format!("{}",hexagonal_tiling.radius().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::CenterX { min, max, step } => ("x center in mm".to_string(), format!("{}",hexagonal_tiling.center().x.get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::CenterY { min, max, step } => ("y center in mm".to_string(), format!("{}",hexagonal_tiling.center().y.get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }
        },
        PosDistType::Hexapolar(hexapolar) => {
            match param{
                PosDistParam::Rings { min, max, step } =>  ("Number of rings".to_string(), format!("{}",hexapolar.nr_of_rings()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::Radius { min, max, step } => ("Radius in mm".to_string(), format!("{}",hexapolar.radius().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }
        },
        PosDistType::FibonacciRectangle(fibonacci_rectangle) => {
            match param{
                PosDistParam::LengthX { min, max, step } => ("x length in mm".to_string(), format!("{}",fibonacci_rectangle.side_length_x().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::LengthY { min, max, step } => ("y length in mm".to_string(), format!("{}",fibonacci_rectangle.side_length_y().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsX { min, max, step } => ("#Points".to_string(), format!("{}",fibonacci_rectangle.nr_of_points()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }},
        PosDistType::FibonacciEllipse(fibonacci_ellipse) => {
            match param{
                PosDistParam::LengthX { min, max, step } => ("x length in mm".to_string(), format!("{}",fibonacci_ellipse.radius_x().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::LengthY { min, max, step } => ("y length in mm".to_string(), format!("{}",fibonacci_ellipse.radius_y().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsX { min, max, step } => ("#Points".to_string(), format!("{}",fibonacci_ellipse.nr_of_points()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }},
        PosDistType::Sobol(sobol_dist) => {
            match param{
                PosDistParam::LengthX { min, max, step } => ("x length in mm".to_string(), format!("{}",sobol_dist.side_length_x().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::LengthY { min, max, step } => ("y length in mm".to_string(), format!("{}",sobol_dist.side_length_y().get::<millimeter>()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                PosDistParam::PointsX { min, max, step } => ("#Points".to_string(), format!("{}",sobol_dist.nr_of_points()), true, format!("{min}"), format!("{max}"), format!("{step}")),
                _ => ("".to_string(), "".to_string(), false, "".to_string(), "".to_string(), "".to_string())
            }},
    };

    rsx!{
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            input {
                class: "form-control bg-dark text-light form-control-sm",
                r#type: "number",
                step: step,
                min: min,
                max: max,
                id: name.clone(),
                name: name.clone(),
                placeholder: place_holder.clone(),
                value: val,
                "readonly": false,
                onchange: {
                    let pos_dist_type = pos_dist_type.clone();
                    move |e: Event<FormData>| {
                        let mut pos_dist_type = pos_dist_type.clone();
                         match &mut pos_dist_type{
                            PosDistType::Random(random) => {
                                match param{
                                    PosDistParam::LengthX { ..  } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            random.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::LengthY { ..  } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            random.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsX { ..  } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            random.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::Grid(grid) => {
                                match param{
                                    PosDistParam::LengthX { ..  } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            grid.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::LengthY { ..  } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            grid.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsX { ..  } => {
                                        if let Ok(points_x) = e.data.parsed::<usize>() {
                                            grid.set_nr_of_points_x(points_x);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsY { ..  } => {
                                        if let Ok(points_y) = e.data.parsed::<usize>() {
                                            grid.set_nr_of_points_y(points_y);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::HexagonalTiling(hexagonal_tiling) => {
                                match param{
                                    PosDistParam::Rings { ..} => {
                                        if let Ok(nr_of_rings) = e.data.parsed::<u8>() {
                                            hexagonal_tiling.set_nr_of_hex_along_radius(nr_of_rings);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::Radius {.. } => {
                                        if let Ok(radius) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_radius(millimeter!(radius));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::CenterX {.. } => {
                                        if let Ok(cx) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_center_x(millimeter!(cx));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::CenterY { .. } => {
                                        if let Ok(cy) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_center_y(millimeter!(cy));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::Hexapolar(hexapolar) => {
                                match param{
                                    PosDistParam::Rings { .. } => {
                                        if let Ok(nr_of_rings) = e.data.parsed::<u8>() {
                                            hexapolar.set_nr_of_rings(nr_of_rings);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::Radius { min, max, step } => {
                                        if let Ok(radius) = e.data.parsed::<f64>() {
                                            hexapolar.set_radius(millimeter!(radius));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::FibonacciRectangle(fibonacci_rectangle) => {
                                match param{
                                    PosDistParam::LengthX { ..  } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            fibonacci_rectangle.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::LengthY { ..  } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            fibonacci_rectangle.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsX { ..  } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            fibonacci_rectangle.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::FibonacciEllipse(fibonacci_ellipse) => {
                                match param{
                                    PosDistParam::LengthX { ..  } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            fibonacci_ellipse.set_radius_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::LengthY { ..  } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            fibonacci_ellipse.set_radius_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsX { ..  } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            fibonacci_ellipse.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                            PosDistType::Sobol(sobol_dist) => {
                                match param{
                                    PosDistParam::LengthX { ..  } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            sobol_dist.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::LengthY { ..  } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            sobol_dist.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    PosDistParam::PointsX { ..  } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            sobol_dist.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    },
                                    _ => todo!()
                                }
                            },
                        }
                        
                    }
                },
            }
            label { class: "form-label text-secondary", r#for: name, {place_holder.clone()} }
        }
    }
}