pub mod input_components;

use dioxus::prelude::*;
use opossum_backend::{RotationAxis, TranslationAxis};
use strum::IntoEnumIterator;

use crate::components::node_editor::CallbackWrapper;
use std::{fmt::Display, str::FromStr};

pub trait IntoInputData<T, D, B>: Into<InputParam>
where
    D: Into<B> + Clone + 'static,
    T: Clone + FromStr + 'static,
    Self: IntoEnumIterator + IntoInputDataStrings<D> + Copy + 'static,
{
    fn setter_from_obj(&self) -> impl FnMut(&mut D, T);

    fn create_callback(&self, mut obj: D, mut sig: Signal<B>) -> CallbackWrapper {
        let this = *self;

        CallbackWrapper::new(move |e: Event<FormData>| {
            if let Some(value) = this.parse_value(e) {
                let mut setter = this.setter_from_obj();
                setter(&mut obj, value);
                sig.set(obj.clone().into());
            }
        })
    }
    fn parse_value(&self, e: Event<FormData>) -> Option<T> {
        let e_value = e.value();
        e_value.parse::<T>().ok()
    }

    fn to_input_data(&self, obj: D, sig: Signal<B>) -> InputData {
        let value_str = self.create_value_string(&obj);
        InputData::new(
            Into::<InputParam>::into(*self),
            &self.create_id_string(),
            self.create_callback(obj, sig),
            value_str,
        )
    }

    fn to_input_data_vec(obj: &D, sig: Signal<B>) -> Vec<InputData> {
        let mut input_data = Vec::<InputData>::new();
        for enum_variant in Self::iter() {
            input_data.push(enum_variant.to_input_data(obj.clone(), sig));
        }
        input_data
    }
}

pub trait IntoInputDataStrings<D> {
    fn create_value_string(&self, obj: &D) -> String;
    fn create_id_string(&self) -> String;
}

#[derive(Clone, PartialEq, Copy, Eq)]
pub enum InputParam {
    Usize(&'static str),
    U8(&'static str),
    F64(&'static str),
    Length(&'static str),
    Energy(&'static str),
    Angle(&'static str),
    Bool(&'static str),
    FilePath(&'static str, &'static str),
}

impl InputParam {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Usize(label)
            | Self::U8(label)
            | Self::F64(label)
            | Self::Length(label)
            | Self::Energy(label)
            | Self::Angle(label)
            | Self::Bool(label)
            | Self::FilePath(label, _) => label,
        }
    }
    #[must_use]
    pub const fn rtype(self) -> &'static str {
        match self {
            Self::Usize(_)
            | Self::U8(_)
            | Self::F64(_)
            | Self::Length(_)
            | Self::Energy(_)
            | Self::Angle(_) => "number",
            Self::Bool(_) => "checkbox",
            Self::FilePath(_, _) => "file",
        }
    }

    #[must_use]
    pub fn id_str(self) -> String {
        self.label().trim().to_string()
    }
}

impl From<TranslationAxis> for InputParam {
    fn from(axis: TranslationAxis) -> Self {
        match axis {
            TranslationAxis::X => Self::Length("X translation in mm"),
            TranslationAxis::Y => Self::Length("Y translation in mm"),
            TranslationAxis::Z => Self::Length("Z translation in mm"),
        }
    }
}

impl From<RotationAxis> for InputParam {
    fn from(axis: RotationAxis) -> Self {
        match axis {
            RotationAxis::Roll => Self::Angle("Roll in degrees"),
            RotationAxis::Pitch => Self::Angle("Pitch in degrees"),
            RotationAxis::Yaw => Self::Angle("Yaw in degrees"),
        }
    }
}

pub fn select_options_from_enum_iterator<T: IntoEnumIterator + Display>(
    active_selection: &T,
    exclude: Option<&[&T]>,
) -> Vec<(bool, String)> {
    let mut options = Vec::<(bool, String)>::new();

    for enum_variant in T::iter() {
        if std::mem::discriminant(&enum_variant) == std::mem::discriminant(active_selection) {
            options.push((true, format!("{enum_variant}")));
        } else if !exclude.is_some_and(|ex| {
            ex.iter()
                .any(|e| std::mem::discriminant(*e) == std::mem::discriminant(&enum_variant))
        }) {
            options.push((false, format!("{enum_variant}")));
        }
    }
    options
}

#[derive(Clone, PartialEq)]
pub struct InputData {
    pub value: String,
    pub id: String,
    pub dist_param: InputParam,
    pub callback_opt: CallbackWrapper,
}

impl InputData {
    pub fn new(
        dist_param: InputParam,
        dist_type: &impl Display,
        callback_opt: CallbackWrapper,
        value: String,
    ) -> Self {
        Self {
            value,
            id: format!("node{dist_type}{}Input", dist_param.id_str()),
            dist_param,
            callback_opt,
        }
    }
}
