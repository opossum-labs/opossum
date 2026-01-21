pub mod input_components;
use dioxus::prelude::*;
use opossum_core::utils::geom_transformation::{RotationAxis, TranslationAxis};
use std::{fmt::Display, str::FromStr};
use strum::IntoEnumIterator;

pub trait IntoInputData<T, D, B: 'static>: Into<InputParam>
where
    D: Into<B> + Clone + 'static,
    T: Clone + FromStr + 'static,
    Self: IntoEnumIterator + IntoInputDataStrings<D> + Copy + 'static,
{
    fn setter_from_obj(&self) -> impl FnMut(&mut D, T);
    fn create_callback(&self, mut obj: D, mut sig: Signal<B>) -> EventHandler<Event<FormData>> {
        let this = *self;
        EventHandler::new(move |e: Event<FormData>| {
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
            self.create_id_string().as_str(),
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

#[derive(Clone, PartialEq, Eq)]
pub enum InputParam {
    Usize(String),
    U8(String),
    F64(String),
    Length(String),
    Selection(String, Vec<(bool, String)>),
    Energy(String),
    Angle(String),
    Bool(String),
    FilePath(String, String),
}

impl InputParam {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Usize(label)
            | Self::U8(label)
            | Self::Selection(label, _)
            | Self::F64(label)
            | Self::Length(label)
            | Self::Energy(label)
            | Self::Angle(label)
            | Self::Bool(label)
            | Self::FilePath(label, _) => label.clone(),
        }
    }
    #[must_use]
    pub const fn rtype(&self) -> &'static str {
        match self {
            Self::Usize(_)
            | Self::U8(_)
            | Self::F64(_)
            | Self::Length(_)
            | Self::Energy(_)
            | Self::Angle(_) => "number",
            Self::Bool(_) => "checkbox",
            Self::FilePath(_, _) => "file",
            Self::Selection(_, _) => "select",
        }
    }

    #[must_use]
    pub fn id_str(&self) -> String {
        let mut label = self.label();
        label.retain(|c| !c.is_whitespace());
        label
    }
}

impl From<TranslationAxis> for InputParam {
    fn from(axis: TranslationAxis) -> Self {
        match axis {
            TranslationAxis::X => Self::Length("X translation in mm".into()),
            TranslationAxis::Y => Self::Length("Y translation in mm".into()),
            TranslationAxis::Z => Self::Length("Z translation in mm".into()),
        }
    }
}

impl From<RotationAxis> for InputParam {
    fn from(axis: RotationAxis) -> Self {
        match axis {
            RotationAxis::Roll => Self::Angle("Roll in degrees".into()),
            RotationAxis::Pitch => Self::Angle("Pitch in degrees".into()),
            RotationAxis::Yaw => Self::Angle("Yaw in degrees".into()),
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
    pub input_param: InputParam,
    pub callback: EventHandler<Event<FormData>>,
    pub readonly: bool,
}

impl InputData {
    pub fn new(
        input_param: InputParam,
        id_str_add_on: &str,
        callback: EventHandler<Event<FormData>>,
        value: String,
    ) -> Self {
        Self {
            value,
            id: format!("{}{}", id_str_add_on, input_param.id_str()),
            input_param,
            callback,
            readonly: false,
        }
    }
}
