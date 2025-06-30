#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;
use opossum_backend::{
    light_data_builder::LightDataBuilder, millimeter, ray_data_builder::RayDataBuilder,
};
use uom::si::length::millimeter;

use crate::components::node_editor::{
    accordion::{LabeledInput, LabeledSelect},
    source_editor::{CallbackWrapper, LightDataBuilderHistory},
};
use strum::EnumIter;
use strum::IntoEnumIterator;

/// A convenience struct representing the current ray type selection in the GUI state.
///
/// It stores the selected [`RayDataBuilder`] variant and provides boolean flags
/// to indicate the selected ray type. This allows for easy querying and updating
/// of the ray type in a user interface context.
#[allow(clippy::struct_excessive_bools)]
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

    pub fn get_option_elements(&self) -> Vec<(bool, String)> {
        let mut option_vals = Vec::<(bool, String)>::new();
        for ray_type in RayDataBuilder::iter() {
            match ray_type {
                RayDataBuilder::Collimated { .. } => {
                    option_vals.push((self.collimated, "Collimated".to_string()))
                }
                RayDataBuilder::PointSrc { .. } => {
                    option_vals.push((self.point_src, "Point Source".to_string()))
                }
                RayDataBuilder::Image { .. } => option_vals.push((self.image, "Image".to_string())),
                RayDataBuilder::Raw { .. } => {}
            }
        }
        option_vals
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
                onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                    let mut point_src = point_src.clone();
                    if let Ok(ref_length) = e.data.parsed::<f64>() {
                        point_src.set_reference_length(millimeter!(ref_length));
                        light_data_builder_sig
                            .with_mut(|ldb| {
                                if let Some(
                                    LightDataBuilder::Geometric(RayDataBuilder::PointSrc(p)),
                                ) = ldb.get_current_mut()
                                {
                                    *p = point_src;
                                }
                            });
                    }
                }),
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
pub fn RayDataBuilderSelector(light_data_builder_sig: Signal<LightDataBuilderHistory>) -> Element {
    light_data_builder_sig
        .read()
        .get_current()
        .map_or(rsx! {}, |ldb| {
            RayTypeSelection::try_from(ldb.clone()).map_or(rsx! {}, |rts| {
                rsx! {
                    LabeledSelect {
                        id: "selectRaySourceType",
                        label: "Rays Type",
                        options: rts.get_option_elements(),
                        onchange: move |e: Event<FormData>| {
                            light_data_builder_sig
                                .with_mut(|ldb| {
                                    let value = e.value();
                                    ldb.set_current_or_default(value.as_str());
                                });
                        },
                    }
                }
            })
        })
}
