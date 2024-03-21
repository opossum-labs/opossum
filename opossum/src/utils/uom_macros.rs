//! Module for additional uom macros that facilitate the creation of Points, vecs or single unit values

/// helper macro
#[macro_export]
macro_rules! uom_unit_creator {

    ($unit:ident, $unit_type:ident, $val1:expr) => {
        $unit_type::new::<$unit>($val1)
    };
    ($unit:ident, $unit_type:ident, $val1:expr, $val2:expr) => {
        Point2::new(
            $unit_type::new::<$unit>($val1),
            $unit_type::new::<$unit>($val2))

    };
    ($unit:ident, $unit_type:ident, $val1:expr, $val2:expr, $val3:expr) => {
        Point3::new(
            $unit_type::new::<$unit>($val1),
            $unit_type::new::<$unit>($val2),
            $unit_type::new::<$unit>($val3))
    };
    ($unit:ident, $unit_type:ident, $( $x:expr ),*) => {
        {
            let mut temp_vec = Vec::new();
            $(
                temp_vec.push($unit_type::new::<$unit>($x));
            )*
            temp_vec
        }
    };
}

///macro to create a Length in kilometer
#[macro_export]
macro_rules! kilometer {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![kilometer, Length, $( $x ),*]
    };
}
///macro to create a Length in meter
#[macro_export]
macro_rules! meter {

    ($( $x:expr ),*) =>{
        {
            use uom::si::length::meter as met;
            $crate::uom_unit_creator![met, Length, $( $x ),*]
        }
    };
}
///macro to create a Length in centimeter
#[macro_export]
macro_rules! centimeter {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![centimeter, Length, $( $x ),*]
    };
}
///macro to create a Length in millimeter
#[macro_export]
macro_rules! millimeter {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![millimeter, Length, $( $x ),*]
    };
}
///macro to create a Length in micrometer
#[macro_export]
macro_rules! micrometer {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![micrometer, Length, $( $x ),*]
    };
}
///macro to create a Length in nanometer
#[macro_export]
macro_rules! nanometer {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![nanometer, Length, $( $x ),*]
    };
}
///macro to create a Length in picometer
#[macro_export]
macro_rules! picometer {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![picometer, Length, $( $x ),*]
    };
}
///macro to create a Length in femtometer
#[macro_export]
macro_rules! femtometer {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![femtometer, Length, $( $x ),*]
    };
}

///macro to create an energy in Terajoule
#[macro_export]
macro_rules! terajoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![terajoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in Gigajoule
#[macro_export]
macro_rules! gigajoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![gigajoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in Megajoule
#[macro_export]
macro_rules! megajoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![megajoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in kilojoule
#[macro_export]
macro_rules! kilojoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![kilojoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in joule
#[macro_export]
macro_rules! joule {
    ($( $x:expr ),*) =>{
        {
            use uom::si::energy::joule as j;
            $crate::uom_unit_creator![j, Energy, $( $x ),*]
        }
    };
}
///macro to create an energy in millijoule
#[macro_export]
macro_rules! millijoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![millijoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in microjoule
#[macro_export]
macro_rules! microjoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![microjoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in nanojoule
#[macro_export]
macro_rules! nanojoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![nanojoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in picojoule
#[macro_export]
macro_rules! picojoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![picojoule, Energy, $( $x ),*]
    };
}
///macro to create an energy in femtojoule
#[macro_export]
macro_rules! femtojoule {
    ($( $x:expr ),*) =>{
        $crate::uom_unit_creator![femtojoule, Energy, $( $x ),*]
    };
}
