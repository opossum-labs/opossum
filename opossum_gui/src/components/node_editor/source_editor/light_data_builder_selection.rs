use crate::{components::node_editor::accordion::LabeledSelect, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::{
    energy_data_builder::EnergyDataBuilder,
    light_data_builder::LightDataBuilder,
    ray_data_builder::{CollimatedSrc, PointSrc, RayDataBuilder},
    PosDistType,
};
use std::collections::HashMap;

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
