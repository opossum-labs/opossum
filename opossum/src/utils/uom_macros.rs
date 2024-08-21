#![warn(missing_docs)]
//! Module for additional uom macros that facilitate the creation of Points, vecs or single unit values

/// helper macro to create the units
#[macro_export]
macro_rules! uom_unit_creator {

    ($unit:ident, $unit_type:ident, $val1:expr) => {
        $unit_type::new::<$unit>($val1)
    };
    ($unit:ident, $unit_type:ident, $val1:expr, $val2:expr) => {
        {
            use nalgebra::Point2;
        Point2::new(
            $unit_type::new::<$unit>($val1),
            $unit_type::new::<$unit>($val2))
        }

    };
    ($unit:ident, $unit_type:ident, $val1:expr, $val2:expr, $val3:expr) => {
        {
        use nalgebra::Point3;
        Point3::new(
            $unit_type::new::<$unit>($val1),
            $unit_type::new::<$unit>($val2),
            $unit_type::new::<$unit>($val3))
        }
    };
    ($unit:ident, $unit_type:ident, $( $x:expr ),*) => {
        {
            use std::vec::Vec;
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
        {
        use uom::si::{f64::Length, length::kilometer};
        $crate::uom_unit_creator![kilometer, Length, $( $x ),*]
        }
    };
}
///macro to create a Length in meter
#[macro_export]
macro_rules! meter {

    ($( $x:expr ),*) =>{
        {
            use uom::si::{f64::Length, length::meter};
            $crate::uom_unit_creator![meter, Length, $( $x ),*]
        }
    };
}
///macro to create a Length in centimeter
#[macro_export]
macro_rules! centimeter {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::centimeter};
        $crate::uom_unit_creator![centimeter, Length, $( $x ),*]
    }};
}
///macro to create a Length in millimeter
#[macro_export]
macro_rules! millimeter {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::millimeter};
        $crate::uom_unit_creator![millimeter, Length, $( $x ),*]
    }};
}
///macro to create a Length in micrometer
#[macro_export]
macro_rules! micrometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::micrometer};
        $crate::uom_unit_creator![micrometer, Length, $( $x ),*]
    }};
}
///macro to create a Length in nanometer
#[macro_export]
macro_rules! nanometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::nanometer};
        $crate::uom_unit_creator![nanometer, Length, $( $x ),*]
    }};
}
///macro to create a Length in picometer
#[macro_export]
macro_rules! picometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::picometer};
        $crate::uom_unit_creator![picometer, Length, $( $x ),*]
    }};
}
///macro to create a Length in femtometer
#[macro_export]
macro_rules! femtometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::femtometer};
        $crate::uom_unit_creator![femtometer, Length, $( $x ),*]
    }};
}
///macro to create a Length in attometer
#[macro_export]
macro_rules! attometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::femtometer};
        $crate::uom_unit_creator![attometer, Length, $( $x ),*]
    }};
}
///macro to create a Length in zeptometer
#[macro_export]
macro_rules! zeptometer {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Length, length::femtometer};
        $crate::uom_unit_creator![zeptometer, Length, $( $x ),*]
    }};
}

///macro to create an energy in Terajoule
#[macro_export]
macro_rules! terajoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::terajoule};
        $crate::uom_unit_creator![terajoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in Gigajoule
#[macro_export]
macro_rules! gigajoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::gigajoule};
        $crate::uom_unit_creator![gigajoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in Megajoule
#[macro_export]
macro_rules! megajoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::megajoule};
        $crate::uom_unit_creator![megajoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in kilojoule
#[macro_export]
macro_rules! kilojoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::kilojoule};
        $crate::uom_unit_creator![kilojoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in joule
#[macro_export]
macro_rules! joule {
    ($( $x:expr ),*) =>{{
        {
            use uom::si::{f64::Energy, energy::joule};
            $crate::uom_unit_creator![joule, Energy, $( $x ),*]
        }
    }};
}
///macro to create an energy in millijoule
#[macro_export]
macro_rules! millijoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::millijoule};
        $crate::uom_unit_creator![millijoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in microjoule
#[macro_export]
macro_rules! microjoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::microjoule};
        $crate::uom_unit_creator![microjoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in nanojoule
#[macro_export]
macro_rules! nanojoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::nanojoule};
        $crate::uom_unit_creator![nanojoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in picojoule
#[macro_export]
macro_rules! picojoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::picojoule};
        $crate::uom_unit_creator![picojoule, Energy, $( $x ),*]
    }};
}
///macro to create an energy in femtojoule
#[macro_export]
macro_rules! femtojoule {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Energy, energy::femtojoule};
        $crate::uom_unit_creator![femtojoule, Energy, $( $x ),*]
    }};
}

///macro to create an angle in radian
#[macro_export]
macro_rules! radian {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::radian};
        $crate::uom_unit_creator![radian, Angle, $( $x ),*]
    }};
}

///macro to create an angle in milliradian
#[macro_export]
macro_rules! milliradian {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::radian};
        $crate::uom_unit_creator![radian, Angle, $( 1e-3*$x ),*]
    }};
}

///macro to create an angle in microradian
#[macro_export]
macro_rules! microradian {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::radian};
        $crate::uom_unit_creator![radian, Angle, $( 1e-6*$x ),*]
    }};
}
///macro to create an angle in minute
#[macro_export]
macro_rules! minute {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::minute};
        $crate::uom_unit_creator![minute, Angle, $( $x ),*]
    }};
}
///macro to create an angle in second
#[macro_export]
macro_rules! second {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::second};
        $crate::uom_unit_creator![second, Angle, $( $x ),*]
    }};
}
///macro to create an angle in degree
#[macro_export]
macro_rules! degree {
    ($( $x:expr ),*) =>{{
        use uom::si::{f64::Angle, angle::degree};
        $crate::uom_unit_creator![degree, Angle, $( $x ),*]
    }};
}

///macro to create a fluence in terajoule per square meter
#[macro_export]
macro_rules! TJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::terajoule_per_square_meter};
        $crate::uom_unit_creator![terajoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in terajoule per square centimeter
#[macro_export]
macro_rules! TJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::terajoule_per_square_centimeter};
        $crate::uom_unit_creator![terajoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in terajoule per square millimeter
#[macro_export]
macro_rules! TJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::terajoule_per_square_millimeter};
        $crate::uom_unit_creator![terajoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square meter
#[macro_export]
macro_rules! GJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::gigajoule_per_square_meter};
        $crate::uom_unit_creator![gigajoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square centimeter
#[macro_export]
macro_rules! GJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::gigajoule_per_square_centimeter};
        $crate::uom_unit_creator![gigajoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square millimeter
#[macro_export]
macro_rules! GJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::gigajoule_per_square_millimeter};
        $crate::uom_unit_creator![gigajoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square meter
#[macro_export]
macro_rules! MJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::megajoule_per_square_meter};
        $crate::uom_unit_creator![megajoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square centimeter
#[macro_export]
macro_rules! MJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::megajoule_per_square_centimeter};
        $crate::uom_unit_creator![megajoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in megajoule per square millimeter
#[macro_export]
macro_rules! MJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::megajoule_per_square_millimeter};
        $crate::uom_unit_creator![megajoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in kilojoule per square meter
#[macro_export]
macro_rules! kJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::kilojoule_per_square_meter};
        $crate::uom_unit_creator![kilojoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in kilojoule per square centimeter
#[macro_export]
macro_rules! kJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::kilojoule_per_square_centimeter};
        $crate::uom_unit_creator![kilojoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in kilojoule per square millimeter
#[macro_export]
macro_rules! kJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::kilojoule_per_square_millimeter};
        $crate::uom_unit_creator![kilojoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in joule per square meter
#[macro_export]
macro_rules! J_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::joule_per_square_meter};
        $crate::uom_unit_creator![joule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in joule per square centimeter
#[macro_export]
macro_rules! J_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::joule_per_square_centimeter};
        $crate::uom_unit_creator![joule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in joule per square millimeter
#[macro_export]
macro_rules! J_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::joule_per_square_millimeter};
        $crate::uom_unit_creator![joule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in millijoule per square meter
#[macro_export]
macro_rules! mJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::millijoule_per_square_meter};
        $crate::uom_unit_creator![millijoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in millijoule per square centimeter
#[macro_export]
macro_rules! mJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::millijoule_per_square_centimeter};
        $crate::uom_unit_creator![millijoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in millijoule per square millimeter
#[macro_export]
macro_rules! mJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::millijoule_per_square_millimeter};
        $crate::uom_unit_creator![millijoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in microjoule per square meter
#[macro_export]
macro_rules! microJ_per_m2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::microjoule_per_square_meter};
        $crate::uom_unit_creator![microjoule_per_square_meter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in microjoule per square centimeter
#[macro_export]
macro_rules! microJ_per_cm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::microjoule_per_square_centimeter};
        $crate::uom_unit_creator![microjoule_per_square_centimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a fluence in microjoule per square millimeter
#[macro_export]
macro_rules! microJ_per_mm2 {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::RadiantExposure, radiant_exposure::microjoule_per_square_millimeter};
        $crate::uom_unit_creator![microjoule_per_square_millimeter, RadiantExposure, $( $x ),*]
        }
    };
}

///macro to create a linear density in 1 per meter
#[macro_export]
macro_rules! num_per_m {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::LinearNumberDensity, linear_number_density::per_meter};
        $crate::uom_unit_creator![per_meter, LinearNumberDensity, $( $x ),*]
        }
    };
}

///macro to create a line density in 1 per centimeter
#[macro_export]
macro_rules! num_per_cm {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::LinearNumberDensity, linear_number_density::per_centimeter};
        $crate::uom_unit_creator![per_centimeter, LinearNumberDensity, $( $x ),*]
        }
    };
}

///macro to create a line density in 1 per millimeter
#[macro_export]
macro_rules! num_per_mm {
    ($( $x:expr ),*) =>{
        {
        use uom::si::{f64::LinearNumberDensity, linear_number_density::per_millimeter};
        $crate::uom_unit_creator![per_millimeter, LinearNumberDensity, $( $x ),*]
        }
    };
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;
    use nalgebra::{Point2, Point3};
    use uom::si::{angle::radian, f64::Length, length::meter};

    #[test]
    fn milliradian_test() {
        let rad = milliradian!(3.);
        assert_relative_eq!(rad.get::<radian>(), 3e-3);
    }
    #[test]
    fn microradian_test() {
        let rad = microradian!(3.);
        assert_relative_eq!(rad.get::<radian>(), 3e-6);
    }
    #[test]
    fn uom_unit_creator() {
        let meter1 = Length::new::<meter>(1.);
        let meter2 = uom_unit_creator!(meter, Length, 1.);
        assert_relative_eq!(meter1.value, meter2.value);

        let meterp12 = Point2::new(Length::new::<meter>(1.), Length::new::<meter>(2.));
        let meterp22 = uom_unit_creator!(meter, Length, 1., 2.);
        assert_relative_eq!(meterp12.x.value, meterp22.x.value);
        assert_relative_eq!(meterp12.y.value, meterp22.y.value);

        let meterp13 = Point3::new(
            Length::new::<meter>(1.),
            Length::new::<meter>(2.),
            Length::new::<meter>(3.),
        );
        let meterp23 = uom_unit_creator!(meter, Length, 1., 2., 3.);
        assert_relative_eq!(meterp13.x.value, meterp23.x.value);
        assert_relative_eq!(meterp13.y.value, meterp23.y.value);
        assert_relative_eq!(meterp13.z.value, meterp23.z.value);

        let meterp14 = vec![
            Length::new::<meter>(1.),
            Length::new::<meter>(2.),
            Length::new::<meter>(3.),
            Length::new::<meter>(4.),
        ];
        let meterp24 = uom_unit_creator!(meter, Length, 1., 2., 3., 4.);
        assert_relative_eq!(meterp14[0].value, meterp24[0].value);
        assert_relative_eq!(meterp14[1].value, meterp24[1].value);
        assert_relative_eq!(meterp14[2].value, meterp24[2].value);
        assert_relative_eq!(meterp14[3].value, meterp24[3].value);

        let meterp15 = vec![
            Length::new::<meter>(1.),
            Length::new::<meter>(2.),
            Length::new::<meter>(3.),
            Length::new::<meter>(4.),
            Length::new::<meter>(5.),
        ];
        let meterp25 = uom_unit_creator!(meter, Length, 1., 2., 3., 4., 5.);
        assert_relative_eq!(meterp15[0].value, meterp25[0].value);
        assert_relative_eq!(meterp15[1].value, meterp25[1].value);
        assert_relative_eq!(meterp15[2].value, meterp25[2].value);
        assert_relative_eq!(meterp15[3].value, meterp25[3].value);
        assert_relative_eq!(meterp15[4].value, meterp25[4].value);
    }
}
