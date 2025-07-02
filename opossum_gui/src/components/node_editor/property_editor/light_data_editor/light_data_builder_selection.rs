#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{components::node_editor::inputs::input_components::LabeledSelect, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::{
    energy_data_builder::EnergyDataBuilder,
    light_data_builder::LightDataBuilder,
    ray_data_builder::{CollimatedSrc, ImageSrc, PointSrc, RayDataBuilder},
    EnergyDistType, PosDistType, SpecDistType,
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
    /// Returns a reference to the currently active [`LightDataBuilder`] configuration.
    ///
    /// This allows read-only access to the builder that is currently selected in the history.
    ///
    /// # Returns
    /// - `Some(&LightDataBuilder)` if a valid entry is selected.
    /// - `None` if the current key does not exist.
    #[must_use]
    pub fn get_current(&self) -> Option<&LightDataBuilder> {
        self.hist.get(&self.current)
    }

    /// Returns a mutable reference to the currently active [`LightDataBuilder`].
    ///
    /// Enables modifying the builder that is currently active in the history.
    ///
    /// # Returns
    /// - `Some(&mut LightDataBuilder)` if the current key exists.
    /// - `None` if the current key is invalid.
    pub fn get_current_mut(&mut self) -> Option<&mut LightDataBuilder> {
        self.hist.get_mut(&self.current)
    }

    /// Returns the key string of the currently selected builder.
    ///
    /// # Returns
    /// A string slice of the current key.
    #[must_use]
    pub const fn get_current_key(&self) -> &str {
        self.current.as_str()
    }

    /// Sets the current builder selection to the entry with the given key.
    ///
    /// # Parameters
    /// - `key`: The key of the entry to make active.
    ///
    /// # Returns
    /// - `true` if the key exists and selection was updated.
    /// - `false` if the key does not exist in the history.
    pub fn set_current(&mut self, key: &str) -> bool {
        if self.hist.contains_key(key) {
            key.clone_into(&mut self.current);
            true
        } else {
            false
        }
    }

    /// Returns a reference to the [`LightDataBuilder`] associated with the given key, if present.
    ///
    /// # Returns
    /// - `Some(&LightDataBuilder)` if the key exists.
    /// - `None` otherwise.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LightDataBuilder> {
        self.hist.get(key)
    }

    /// Inserts a new [`LightDataBuilder`] and sets it as the active one.
    ///
    /// If a builder with the same key already exists, it is replaced.
    ///
    /// # Parameters
    /// - `key`: The identifier for the builder.
    /// - `ld_builder`: The builder to insert.
    pub fn insert_and_set_current(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
        key.clone_into(&mut self.current);
    }

    /// Inserts a [`LightDataBuilder`] under a specified key.
    ///
    /// Overwrites the existing entry if the key already exists.
    pub fn insert(&mut self, key: &str, ld_builder: LightDataBuilder) {
        self.hist.insert(key.to_owned(), ld_builder);
    }

    /// Replaces an existing [`LightDataBuilder`] under the given key or inserts a new one.
    ///
    /// # Parameters
    /// - `key`: The key to insert or replace at.
    /// - `new_ld_builder`: The builder to store.
    pub fn replace_or_insert(&mut self, key: &str, new_ld_builder: &LightDataBuilder) {
        if let Some(ld_builder) = self.hist.get_mut(key) {
            *ld_builder = new_ld_builder.clone();
        } else {
            self.insert(key, new_ld_builder.clone());
        }
    }

    /// Replaces or inserts a [`LightDataBuilder`] and sets it as the current selection.
    ///
    /// This guarantees that the inserted builder becomes the active one, regardless of whether it was already present.
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
        key.clone_into(&mut self.current);
    }

    /// Checks whether the currently selected builder is of type `Geometric` and whether it is collimated.
    ///
    /// # Returns
    /// A tuple:
    /// - `is_geometric`: `true` if the builder is geometric.
    /// - `is_collimated`: `true` if the builder is of type `Collimated`.
    #[must_use]
    pub fn is_rays_is_collimated(&self) -> (bool, bool) {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(_) => (true, true),
                _ => (true, false),
            },
            _ => (false, false),
        }
    }

    // Checks whether the currently selected builder is of type `Geometric` and whether it is of image type.
    ///
    /// # Returns
    /// A tuple:
    /// - `is_geometric`: `true` if the builder is geometric.
    /// - `is_not_image`: `true` if the builder is not of type `Image`.
    #[must_use]
    pub fn is_rays_is_not_image(&self) -> (bool, bool) {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Image(_) => (true, false),
                _ => (true, true),
            },
            _ => (false, false),
        }
    }

    /// Returns the [`RayDataBuilder`] from the current builder, if it is of type `Geometric`.
    ///
    /// # Returns
    /// - `Some(RayDataBuilder)` if current is `LightDataBuilder::Geometric`.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_current_ray_data_builder(&self) -> Option<RayDataBuilder> {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => Some(ray_data_builder.clone()),
            _ => None,
        }
    }

    /// Returns the [`PosDistType`] of the currently selected builder if available.
    ///
    /// Only `Collimated` and `PointSrc` ray types support positional distributions.
    ///
    /// # Returns
    /// - `Some(PosDistType)` if supported.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_current_pos_dist_type(&self) -> Option<PosDistType> {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => Some(*collimated_src.pos_dist()),
                RayDataBuilder::PointSrc(point_src) => Some(*point_src.pos_dist()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the [`EnergyDistType`] of the current builder, if applicable.
    ///
    /// Supported for `Collimated` and `PointSrc` ray types only.
    ///
    /// # Returns
    /// - `Some(EnergyDistType)` if available.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_current_energy_dist_type(&self) -> Option<EnergyDistType> {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => Some(*collimated_src.energy_dist()),
                RayDataBuilder::PointSrc(point_src) => Some(*point_src.energy_dist()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Returns the [`SpecDistType`] of the current builder, if supported.
    ///
    /// Applicable to `Collimated` and `PointSrc` ray types.
    ///
    /// # Returns
    /// - `Some(SpecDistType)` if supported.
    /// - `None` otherwise.
    #[must_use]
    pub fn get_current_spectral_dist_type(&self) -> Option<SpecDistType> {
        match self.get_current() {
            Some(LightDataBuilder::Geometric(ray_data_builder)) => match ray_data_builder {
                RayDataBuilder::Collimated(collimated_src) => {
                    Some(collimated_src.spect_dist().clone())
                }
                RayDataBuilder::PointSrc(point_src) => Some(point_src.spect_dist().clone()),
                _ => None,
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
            }
        }
    }

    /// Sets a new [`EnergyDistType`] for the currently selected ray source,
    /// if it supports energy distributions (`Collimated` or `PointSrc`).
    ///
    /// The updated builder is saved under multiple keys:
    /// - `"Rays"` and the specific ray type (e.g., `"Collimated"`)
    /// - the stringified `EnergyDistType` (used as the new current key)
    ///
    /// Unsupported types are logged.
    ///
    /// # Arguments
    ///
    /// * `new_energy_dist` - The new energy distribution type to assign.
    pub fn set_energy_dist_type(&mut self, new_energy_dist: EnergyDistType) {
        if let Some(rdb) = &mut self.get_current_ray_data_builder() {
            let energy_dist_string = format!("{new_energy_dist}");
            match rdb {
                RayDataBuilder::Collimated(collimated_src) => {
                    collimated_src.set_energy_dist(new_energy_dist);
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        collimated_src.clone(),
                    ));
                    self.replace_or_insert("Collimated", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&energy_dist_string, new_ld_builder);
                }
                RayDataBuilder::PointSrc(point_src) => {
                    point_src.set_energy_dist(new_energy_dist);
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src.clone()));
                    self.replace_or_insert("Point Source", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&energy_dist_string, new_ld_builder);
                }
                _ => {
                    OPOSSUM_UI_LOGS.write().add_log(&format!(
                        "set_pos_dist_type: Unsupported RayDataBuilder type: {rdb}"
                    ));
                }
            }
        }
    }

    /// Sets a new [`SpecDistType`] for the currently selected ray source,
    /// if it supports spectral distributions (`Collimated` or `PointSrc`).
    ///
    /// The updated builder is saved under multiple keys:
    /// - `"Rays"` and the specific ray type (e.g., `"Collimated"`)
    /// - the stringified `SpecDistType` (used as the new current key)
    ///
    /// Unsupported types are logged.
    ///
    /// # Arguments
    ///
    /// * `new_spectral_dist` - The new spectral distribution type to assign.
    pub fn set_spectral_dist_type(&mut self, new_spectral_dist: SpecDistType) {
        if let Some(rdb) = &mut self.get_current_ray_data_builder() {
            let spectral_dist_string = format!("{new_spectral_dist}");
            match rdb {
                RayDataBuilder::Collimated(collimated_src) => {
                    collimated_src.set_spect_dist(new_spectral_dist);
                    let new_ld_builder = LightDataBuilder::Geometric(RayDataBuilder::Collimated(
                        collimated_src.clone(),
                    ));
                    self.replace_or_insert("Collimated", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&spectral_dist_string, new_ld_builder);
                }
                RayDataBuilder::PointSrc(point_src) => {
                    point_src.set_spect_dist(new_spectral_dist);
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::PointSrc(point_src.clone()));
                    self.replace_or_insert("Point Source", &new_ld_builder);
                    self.replace_or_insert("Rays", &new_ld_builder);
                    self.replace_or_insert_and_set_current(&spectral_dist_string, new_ld_builder);
                }
                _ => {
                    OPOSSUM_UI_LOGS.write().add_log(&format!(
                        "set_pos_dist_type: Unsupported RayDataBuilder type: {rdb}"
                    ));
                }
            }
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
                    self.replace_or_insert_and_set_current(key, new_ld_builder);
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
                "Image" => {
                    let new_ld_builder =
                        LightDataBuilder::Geometric(RayDataBuilder::Image(ImageSrc::default()));
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
                "Uniform" | "Generalized Gaussian" => {
                    if let Some(energy_dist_type) = EnergyDistType::default_from_name(key) {
                        self.set_energy_dist_type(energy_dist_type);
                    }
                }

                "Laser Lines" | "Gaussian" => {
                    if let Some(spectral_dist_type) = SpecDistType::default_from_name(key) {
                        self.set_spectral_dist_type(spectral_dist_type);
                    }
                }

                _ => OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Unknown source type: {key}")),
            }
        }
    }
}
impl Default for LightDataBuilderHistory {
    /// Creates a default history with a single entry for `"Rays"` using the default [`RayDataBuilder`].
    fn default() -> Self {
        let current = "Rays".to_owned();
        let ld_builder = LightDataBuilder::Geometric(RayDataBuilder::default());
        let mut hist = HashMap::<String, LightDataBuilder>::new();
        hist.insert(current.clone(), ld_builder);
        Self { hist, current }
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
