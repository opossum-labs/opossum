use std::{collections::HashMap, fmt::Display};

use super::node_editor_component::NodeChange;
use crate::{
    components::node_editor::accordion::{AccordionItem, LabeledInput, LabeledSelect},
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{
    energy_data_builder::EnergyDataBuilder,
    light_data_builder::LightDataBuilder,
    millimeter,
    ray_data_builder::{CollimatedSrc, PointSrc, RayDataBuilder},
    Isometry, PosDistType, Proptype,
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uom::si::length::millimeter;

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
            self.fibonacci_ell,
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

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for pos_dist in PosDistType::iter() {
            option_vals.push(match pos_dist {
                PosDistType::Random(_) => (self.rand, pos_dist.to_string()),
                PosDistType::Grid(_) => (self.grid, pos_dist.to_string()),
                PosDistType::HexagonalTiling(_) => (self.hexagonal, pos_dist.to_string()),
                PosDistType::Hexapolar(_) => (self.hexapolar, pos_dist.to_string()),
                PosDistType::FibonacciRectangle(_) => (self.fibonacci_rect, pos_dist.to_string()),
                PosDistType::FibonacciEllipse(_) => (self.fibonacci_ell, pos_dist.to_string()),
                PosDistType::Sobol(_) => (self.sobol, pos_dist.to_string()),
            })
        }
        option_vals
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

/// A convenience struct representing the current ray type selection in the GUI state.
///
/// It stores the selected [`RayDataBuilder`] variant and provides boolean flags
/// to indicate the selected ray type. This allows for easy querying and updating
/// of the ray type in a user interface context.
struct RayTypeSelection {
    /// The currently selected ray type.
    pub ray_type: RayDataBuilder,
    /// `true` if the selected ray type is `Collimated`.
    pub collimated: bool,
    /// `true` if the selected ray type is `PointSrc`.
    pub point_src: bool,
    /// `true` if the selected ray type is `Raw`.
    pub raw: bool,
    /// `true` if the selected ray type is `Image`.
    pub image: bool,
}

impl RayTypeSelection {
    /// Creates a new [`RayTypeSelection`] from a given [`RayDataBuilder`] variant.
    ///
    /// This function initializes the internal boolean flags to match the type of the provided
    /// `ray_type`.
    ///
    /// # Arguments
    ///
    /// * `ray_type` - A variant of [`RayDataBuilder`] representing the selected ray type.
    ///
    /// # Returns
    ///
    /// A fully initialized `RayTypeSelection` with corresponding flags set.
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
    /// Updates the internal `ray_type` and sets the boolean flags accordingly.
    ///
    /// This function matches the provided `ray_type` and updates the internal state
    /// so that only one of the flags (`collimated`, `point_src`, `raw`, `image`) is `true`.
    ///
    /// # Arguments
    ///
    /// * `ray_type` - A new [`RayDataBuilder`] to set.
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
    /// Tries to construct a [`RayTypeSelection`] from a [`LightDataBuilder`].
    ///
    /// Only works if the provided `LightDataBuilder` is of the `Geometric` variant.
    ///
    /// # Errors
    ///
    /// Returns an error if the `LightDataBuilder` is not of type `Geometric`.
    fn try_from(value: LightDataBuilder) -> Result<Self, Self::Error> {
        match value {
            LightDataBuilder::Geometric(ray_data_builder) => Ok(Self::new(ray_data_builder)),
            _ => Err("Wrong Lightdatabuilder type!".to_owned()),
        }
    }
}

/// Stores the history of [`LightDataBuilder`] instances keyed by string identifiers.
///
/// This structure is used to preserve and restore the state of previously selected
/// attributes when switching between them in a GUI. This avoids resetting everything
/// to default values when navigating back and forth between configurations.
#[derive(Clone, PartialEq)]
pub struct LightDataBuilderHistory {
    /// Internal mapping from string keys (e.g., "Rays", "Collimated") to [`LightDataBuilder`] instances.
    hist: HashMap<String, LightDataBuilder>,
    /// Key of the currently active selection.
    current: String,
}
impl LightDataBuilderHistory {
    /// Creates a default history with a single entry for `"Rays"` using the default [`RayDataBuilder`].
    pub fn default() -> Self {
        let current = "Rays".to_owned();
        let ld_builder = LightDataBuilder::Geometric(RayDataBuilder::default());
        let mut hist = HashMap::<String, LightDataBuilder>::new();
        hist.insert(current.clone(), ld_builder);
        Self { hist, current }
    }
    /// Returns a reference to the currently active [`LightDataBuilder`].
    pub fn get_current(&self) -> &LightDataBuilder {
        self.hist.get(&self.current).unwrap()
    }

    /// Returns a mutable reference to the currently active [`LightDataBuilder`].
    pub fn get_current_mut(&mut self) -> &mut LightDataBuilder {
        self.hist.get_mut(&self.current).unwrap()
    }

    /// Returns the key of the currently active entry.
    pub fn get_current_key(&self) -> &str {
        self.current.as_str()
    }

    /// Attempts to set the active entry to the given key.
    ///
    /// Returns `true` if the key exists and the current selection was updated,
    /// otherwise returns `false`.
    pub fn set_current(&mut self, key: &str) -> bool {
        if let Some(_) = self.hist.get(key) {
            self.current = key.to_owned();
            true
        } else {
            false
        }
    }

    /// Returns a reference to the entry associated with the given key, if it exists.
    pub fn get(&self, key: &str) -> Option<&LightDataBuilder> {
        self.hist.get(key)
    }

    /// Inserts a new [`LightDataBuilder`] and sets it as the current selection.
    ///
    /// If an entry with the same key already exists, it will be overwritten.
    pub fn insert_and_set_current(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
        self.current = key.to_owned();
    }

    /// Inserts a new [`LightDataBuilder`] under the given key.
    ///
    /// Overwrites the existing entry if the key already exists.
    pub fn insert(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
    }

    /// Replaces the [`LightDataBuilder`] at the given key if it exists,
    /// otherwise inserts a new one.
    pub fn replace_or_insert(&mut self, key: &str, new_ld_builder: &LightDataBuilder) {
        if let Some(ld_builder) = self.hist.get_mut(key) {
            *ld_builder = new_ld_builder.clone();
        } else {
            self.insert(key, new_ld_builder.clone());
        }
    }

    /// Replaces or inserts a [`LightDataBuilder`] and sets it as the current entry.
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

    /// Returns a tuple `(is_geometric, is_collimated)` for the current builder.
    ///
    /// - `is_geometric` is `true` if the current builder is a `Geometric` ray source.
    /// - `is_collimated` is `true` if the source is specifically a `Collimated` one.
    pub fn is_rays_is_collimated(&self) -> (bool, bool) {
        match self.get_current() {
            LightDataBuilder::Geometric(ray_data_builder) => match ray_data_builder {
                RayDataBuilder::Collimated(_) => (true, true),
                _ => (true, false),
            },
            _ => (false, false),
        }
    }

    /// Returns the [`RayDataBuilder`] from the current [`LightDataBuilder`] if it is of type `Geometric`.
    ///
    /// # Returns
    ///
    /// `Some(RayDataBuilder)` if the current builder is of type `LightDataBuilder::Geometric`,  
    /// otherwise `None`.
    pub fn get_current_ray_data_builder(&self) -> Option<RayDataBuilder> {
        match self.get_current() {
            LightDataBuilder::Geometric(ray_data_builder) => Some(ray_data_builder.clone()),
            _ => None,
        }
    }

    /// Returns the [`PosDistType`] from the currently selected ray source,
    /// if it supports positional distributions.
    ///
    /// # Returns
    ///
    /// - `Some(PosDistType)` for `Collimated` or `PointSrc` ray types.
    /// - `None` for `Raw`, `Image`, or non-geometric light sources.
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

    /// Sets a new [`PosDistType`] for the currently selected ray source,
    /// if it supports positional distributions (`Collimated` or `PointSrc`).
    ///
    /// The updated builder is saved under multiple keys:
    /// - `"Rays"` and the specific ray type (e.g., `"Collimated"`)
    /// - the stringified `PosDistType` (used as the new current key)
    ///
    /// Unsupported types are logged.
    ///
    /// # Arguments
    ///
    /// * `new_pos_dist` - The new position distribution type to assign.
    pub fn set_pos_dist_type(&mut self, new_pos_dist: PosDistType) {
        if let Some(rdb) = &mut self.get_current_ray_data_builder() {
            let pos_dist_string = format!("{new_pos_dist}");
            match rdb {
                RayDataBuilder::Collimated(collimated_src) => {
                    collimated_src.set_pos_dist(new_pos_dist);
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        collimated_src.clone(),
                    ));
                    self.replace_or_insert("Collimated", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&pos_dist_string, new_ld_builder);
                }
                RayDataBuilder::PointSrc(point_src) => {
                    point_src.set_pos_dist(new_pos_dist);
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src.clone()));
                    self.replace_or_insert("Point Source", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&pos_dist_string, new_ld_builder);
                }
                _ => {
                    OPOSSUM_UI_LOGS.write().add_log(&format!(
                        "set_pos_dist_type: Unsupported RayDataBuilder type: {rdb}"
                    ));
                }
            };
        }
    }

    pub fn set_current_or_default(&mut self, key: &str) {
        if !self.set_current(key) {
            match key {
                "Rays" => {
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        CollimatedSrc::default(),
                    ));
                    self.replace_or_insert_and_set_current(key, new_ld_builder);
                }
                "Energy" => {
                    let new_ld_builder = LightDataBuilder::Energy(EnergyDataBuilder::default());
                    self.replace_or_insert_and_set_current(key, new_ld_builder.clone());
                }
                "Collimated" => {
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        CollimatedSrc::default(),
                    ));
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(key, new_ld_builder);
                }
                "Point Source" => {
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::PointSrc(PointSrc::default()));
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(key, new_ld_builder);
                }
                "Random"
                | "Grid"
                | "Hexagonal"
                | "Hexapolar"
                | "Fibonacci, rectangular"
                | "Fibonacci, elliptical"
                | "Sobol" => {
                    if let Some(pos_dist_type) = PosDistType::default_from_name(key) {
                        self.set_pos_dist_type(pos_dist_type);
                    }
                }

                _ => OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Unknown source type: {}", key)),
            }
        }
    }
}

#[component]
pub fn SourceEditor(
    hidden: bool,
    light_data_builder_opt: Option<Proptype>,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let mut light_data_builder_sig = Signal::new(LightDataBuilderHistory::default());

    use_effect(move || {
        node_change.set(Some(NodeChange::Property(
            "light data".to_owned(),
            serde_json::to_value(Proptype::LightDataBuilder(Some(
                light_data_builder_sig.read().get_current().clone(),
            )))
            .unwrap(),
        )))
    });

    use_effect(move || node_change.set(Some(NodeChange::Isometry(Isometry::identity()))));
    use_effect(move || {
        let (ld_builder, key) = match &light_data_builder_opt {
        Some(Proptype::LightDataBuilder(Some(ld)))
            if matches!(ld, LightDataBuilder::Geometric(_)) =>
        {
            (ld.clone(), "Rays")
        }
        Some(Proptype::LightDataBuilder(Some(ld))) => (ld.clone(), "Energy"),
        _ => (LightDataBuilder::default(), "Rays"),
    };light_data_builder_sig.with_mut(|ldb| ldb.replace_or_insert_and_set_current(key, ld_builder))
});

    let accordion_item_content = rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig }
        RayDataBuilderSelector { light_data_builder_sig }
        ReferenceLengthEditor { light_data_builder_sig }
        DistributionEditor { light_data_builder_sig }
    };
    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Light Source",
            header_id: "sourceHeading",
            parent_id: "accordionNodeConfig",
            content_id: "sourceCollapse",
            hidden,
        }
    }
}

#[component]
pub fn DistributionEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (is_rays, _) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        div {
            hidden: !is_rays,
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceDists",
            PositionDistributionEditor { light_data_builder_sig }
            EnergyDistributionEditor { light_data_builder_sig }
            SpectralDistributionEditor { light_data_builder_sig }
        }
    }
}

#[component]
pub fn ReferenceLengthEditor(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (_, is_collimated) = light_data_builder_sig.read().is_rays_is_collimated();
    if let Some(RayDataBuilder::PointSrc(point_src)) =
        light_data_builder_sig.read().get_current_ray_data_builder()
    {
        rsx! {
            LabeledInput {
                id: "pointsrcRefLength",
                label: "Reference Length in mm",
                value: format!("{}", point_src.reference_length().get::<millimeter>()),
                onchange: move |e: Event<FormData>| {
                    let mut point_src = point_src.clone();
                    if let Ok(ref_length) = e.data.parsed::<f64>() {
                        point_src.set_reference_length(millimeter!(ref_length));
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                if let LightDataBuilder::Geometric(RayDataBuilder::PointSrc(p)) =
                                    ldb.get_current_mut()
                                {
                                    *p = point_src;
                                }
                            });
                    }
                },
                r#type: "number",
                min: "0.0000000001",
                hidden: is_collimated,
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn PositionDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {
        RayPositionDistributionSelector { light_data_builder_sig }
        RayDistributionEditor { light_data_builder_sig }
    };

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Position Distribution",
            header_id: "sourcePositionDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourcePositionDistCollapse",
        }
    }
}

#[component]
pub fn EnergyDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {};

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Energy Distribution",
            header_id: "sourceEnergyDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceEnergyDistCollapse",
        }
    }
}

#[component]
pub fn SpectralDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {};

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Spectral Distribution",
            header_id: "sourceSpectralDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceSpectralDistCollapse",
        }
    }
}

#[component]
pub fn SourceLightDataBuilderSelector(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let (is_rays, _) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        LabeledSelect {
            id: "selectSourceType",
            label: "Source Type",
            options: vec![(!is_rays, "Energy".to_owned()), (is_rays, "Rays".to_owned())],
            onchange: move |e: Event<FormData>| {
                light_data_builder_sig
                    .with_mut(|ldb| {
                        let value = e.value();
                        ldb.set_current_or_default(value.as_str());
                    });
            },
        }
    }
}

#[component]
pub fn RayDataBuilderSelector(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    let (show, is_collimated) = light_data_builder_sig.read().is_rays_is_collimated();

    rsx! {
        LabeledSelect {
            id: "selectRaySourceType",
            label: "Rays Type",
            options: vec![
                (is_collimated, "Collimated".to_owned()),
                (!is_collimated, "Point Source".to_owned()),
            ],
            hidden: !show,
            onchange: move |e: Event<FormData>| {
                light_data_builder_sig
                    .with_mut(|ldb| {
                        let value = e.value();
                        ldb.set_current_or_default(value.as_str());
                    });
            },
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
            LabeledSelect {
                id: "selectRaysPosDistribution",
                label: "Rays Position Distribution",
                options: rpd.get_option_elements(),
                hidden: !show,
                onchange: move |e: Event<FormData>| {
                    light_data_builder_sig
                        .with_mut(|ldb| {
                            let value = e.value();
                            ldb.set_current_or_default(value.as_str());
                        });
                },
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
                        PosDistType::Random(_) => {
                            rsx! {
                                
                                NodePosDistInput {
                                    pos_dist_type,
                                    param: PosDistParam::PointsX {
                                        min: 1,
                                        max: 1000000000,
                                        step: 1,
                                    },
                                    light_data_builder_sig,
                                }
                                
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthX {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthY {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::Grid(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::PointsX {
                                                min: 1,
                                                max: 1000000000,
                                                step: 1,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::PointsY {
                                                min: 1,
                                                max: 1000000000,
                                                step: 1,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthX {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthY {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::HexagonalTiling(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::Rings {
                                                min: 1,
                                                max: 255,
                                                step: 1,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::Radius {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::CenterX {
                                                min: -1e9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::CenterY {
                                                min: -1e9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::Hexapolar(_) => {
                            rsx! {
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::Rings {
                                                min: 1,
                                                max: 255,
                                                step: 1,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::Radius {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::FibonacciRectangle(_) => {
                            rsx! {
                                
                                NodePosDistInput {
                                    pos_dist_type,
                                    param: PosDistParam::PointsX {
                                        min: 1,
                                        max: 1000000000,
                                        step: 1,
                                    },
                                    light_data_builder_sig,
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthX {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthY {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::FibonacciEllipse(_) => {
                            rsx! {
                                
                                NodePosDistInput {
                                    pos_dist_type,
                                    param: PosDistParam::PointsX {
                                        min: 1,
                                        max: 1000000000,
                                        step: 1,
                                    },
                                    light_data_builder_sig,
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthX {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthY {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                        PosDistType::Sobol(sobol_dist) => {
                            rsx! {
                                
                                NodePosDistInput {
                                    pos_dist_type,
                                    param: PosDistParam::PointsX {
                                        min: 1,
                                        max: 1000000000,
                                        step: 1,
                                    },
                                    light_data_builder_sig,
                                }
                                div { class: "row gy-1 gx-2",
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthX {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                    div { class: "col-sm",
                                        NodePosDistInput {
                                            pos_dist_type,
                                            param: PosDistParam::LengthY {
                                                min: 1e-9,
                                                max: 1e9,
                                                step: 1.,
                                            },
                                            light_data_builder_sig,
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum PosDistParam {
    Rings { min: u8, max: u8, step: u8 },
    Radius { min: f64, max: f64, step: f64 },
    CenterX { min: f64, max: f64, step: f64 },
    CenterY { min: f64, max: f64, step: f64 },
    LengthX { min: f64, max: f64, step: f64 },
    LengthY { min: f64, max: f64, step: f64 },
    PointsX { min: usize, max: usize, step: usize },
    PointsY { min: usize, max: usize, step: usize },
}

impl Display for PosDistParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let param = match self {
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
pub fn NodePosDistInput(
    pos_dist_type: PosDistType,
    param: PosDistParam,
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let name = format!("{pos_dist_type}{param}");

    let (place_holder, val, valid, min, max, step) = match pos_dist_type {
        PosDistType::Random(random) => match param {
            PosDistParam::LengthX { min, max, step } => (
                "x length in mm".to_string(),
                format!("{}", random.side_length_x().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::LengthY { min, max, step } => (
                "y length in mm".to_string(),
                format!("{}", random.side_length_y().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsX { min, max, step } => (
                "#Points".to_string(),
                format!("{}", random.nr_of_points()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::Grid(grid) => match param {
            PosDistParam::LengthX { min, max, step } => (
                "x length in mm".to_string(),
                format!("{}", grid.side_length().0.get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::LengthY { min, max, step } => (
                "y length in mm".to_string(),
                format!("{}", grid.side_length().1.get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsX { min, max, step } => (
                "#Points along x".to_string(),
                format!("{}", grid.nr_of_points().0),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsY { min, max, step } => (
                "#Points along x".to_string(),
                format!("{}", grid.nr_of_points().1),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::HexagonalTiling(hexagonal_tiling) => match param {
            PosDistParam::Rings { min, max, step } => (
                "Number of rings".to_string(),
                format!("{}", hexagonal_tiling.nr_of_hex_along_radius()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::Radius { min, max, step } => (
                "Radius in mm".to_string(),
                format!("{}", hexagonal_tiling.radius().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::CenterX { min, max, step } => (
                "x center in mm".to_string(),
                format!("{}", hexagonal_tiling.center().x.get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::CenterY { min, max, step } => (
                "y center in mm".to_string(),
                format!("{}", hexagonal_tiling.center().y.get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::Hexapolar(hexapolar) => match param {
            PosDistParam::Rings { min, max, step } => (
                "Number of rings".to_string(),
                format!("{}", hexapolar.nr_of_rings()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::Radius { min, max, step } => (
                "Radius in mm".to_string(),
                format!("{}", hexapolar.radius().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::FibonacciRectangle(fibonacci_rectangle) => match param {
            PosDistParam::LengthX { min, max, step } => (
                "x length in mm".to_string(),
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_x().get::<millimeter>()
                ),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::LengthY { min, max, step } => (
                "y length in mm".to_string(),
                format!(
                    "{}",
                    fibonacci_rectangle.side_length_y().get::<millimeter>()
                ),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsX { min, max, step } => (
                "#Points".to_string(),
                format!("{}", fibonacci_rectangle.nr_of_points()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::FibonacciEllipse(fibonacci_ellipse) => match param {
            PosDistParam::LengthX { min, max, step } => (
                "x length in mm".to_string(),
                format!("{}", fibonacci_ellipse.radius_x().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::LengthY { min, max, step } => (
                "y length in mm".to_string(),
                format!("{}", fibonacci_ellipse.radius_y().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsX { min, max, step } => (
                "#Points".to_string(),
                format!("{}", fibonacci_ellipse.nr_of_points()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
        PosDistType::Sobol(sobol_dist) => match param {
            PosDistParam::LengthX { min, max, step } => (
                "x length in mm".to_string(),
                format!("{}", sobol_dist.side_length_x().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::LengthY { min, max, step } => (
                "y length in mm".to_string(),
                format!("{}", sobol_dist.side_length_y().get::<millimeter>()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            PosDistParam::PointsX { min, max, step } => (
                "#Points".to_string(),
                format!("{}", sobol_dist.nr_of_points()),
                true,
                format!("{min}"),
                format!("{max}"),
                format!("{step}"),
            ),
            _ => (
                "".to_string(),
                "".to_string(),
                false,
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ),
        },
    };

    rsx! {
        div { class: "form-floating border-start", "data-mdb-input-init": "",
            input {
                class: "form-control bg-dark text-light form-control-sm",
                r#type: "number",
                step,
                min,
                max,
                id: name.clone(),
                name: name.clone(),
                placeholder: place_holder.clone(),
                value: val,
                "readonly": false,
                onchange: {
                    let pos_dist_type = pos_dist_type.clone();
                    move |e: Event<FormData>| {
                        let mut pos_dist_type = pos_dist_type.clone();
                        match &mut pos_dist_type {
                            PosDistType::Random(random) => {
                                match param {
                                    PosDistParam::LengthX { .. } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            random.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::LengthY { .. } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            random.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsX { .. } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            random.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::Grid(grid) => {
                                match param {
                                    PosDistParam::LengthX { .. } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            grid.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::LengthY { .. } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            grid.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsX { .. } => {
                                        if let Ok(points_x) = e.data.parsed::<usize>() {
                                            grid.set_nr_of_points_x(points_x);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsY { .. } => {
                                        if let Ok(points_y) = e.data.parsed::<usize>() {
                                            grid.set_nr_of_points_y(points_y);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::HexagonalTiling(hexagonal_tiling) => {
                                match param {
                                    PosDistParam::Rings { .. } => {
                                        if let Ok(nr_of_rings) = e.data.parsed::<u8>() {
                                            hexagonal_tiling.set_nr_of_hex_along_radius(nr_of_rings);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::Radius { .. } => {
                                        if let Ok(radius) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_radius(millimeter!(radius));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::CenterX { .. } => {
                                        if let Ok(cx) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_center_x(millimeter!(cx));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::CenterY { .. } => {
                                        if let Ok(cy) = e.data.parsed::<f64>() {
                                            hexagonal_tiling.set_center_y(millimeter!(cy));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::Hexapolar(hexapolar) => {
                                match param {
                                    PosDistParam::Rings { .. } => {
                                        if let Ok(nr_of_rings) = e.data.parsed::<u8>() {
                                            hexapolar.set_nr_of_rings(nr_of_rings);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::Radius { min, max, step } => {
                                        if let Ok(radius) = e.data.parsed::<f64>() {
                                            hexapolar.set_radius(millimeter!(radius));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::FibonacciRectangle(fibonacci_rectangle) => {
                                match param {
                                    PosDistParam::LengthX { .. } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            fibonacci_rectangle
                                                .set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::LengthY { .. } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            fibonacci_rectangle
                                                .set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsX { .. } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            fibonacci_rectangle.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::FibonacciEllipse(fibonacci_ellipse) => {
                                match param {
                                    PosDistParam::LengthX { .. } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            fibonacci_ellipse.set_radius_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::LengthY { .. } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            fibonacci_ellipse.set_radius_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsX { .. } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            fibonacci_ellipse.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                            PosDistType::Sobol(sobol_dist) => {
                                match param {
                                    PosDistParam::LengthX { .. } => {
                                        if let Ok(length_x) = e.data.parsed::<f64>() {
                                            sobol_dist.set_side_length_x(millimeter!(length_x));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::LengthY { .. } => {
                                        if let Ok(length_y) = e.data.parsed::<f64>() {
                                            sobol_dist.set_side_length_y(millimeter!(length_y));
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    PosDistParam::PointsX { .. } => {
                                        if let Ok(points) = e.data.parsed::<usize>() {
                                            sobol_dist.set_nr_of_points(points);
                                            light_data_builder_sig
                                                .with_mut(|ldb| { ldb.set_pos_dist_type(pos_dist_type) })
                                        }
                                    }
                                    _ => todo!(),
                                }
                            }
                        }
                    }
                },
            }
            label { class: "form-label text-secondary", r#for: name, {place_holder.clone()} }
        }
    }
}
