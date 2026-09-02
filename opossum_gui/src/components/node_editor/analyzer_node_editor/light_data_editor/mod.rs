pub mod energy_source_editor;
pub mod ray_source_editor;

use opossum_core::{
    distributions::spectral::{Gaussian, LaserLines, SpecDistType},
    joule, nanometer,
    prelude::{EnergyDataBuilder, EnergyLaserLines, RayDataSource},
};
use uom::si::f64::Length;

/// Creates a default `Gaussian` spectral distribution centered at `default_wvl`.
pub fn default_gaussian(default_wvl: Length) -> Gaussian {
    let mut g = Gaussian::default();
    let span = nanometer!(50.0);
    let start = if default_wvl > span {
        default_wvl - span
    } else {
        default_wvl / 2.0
    };
    let end = default_wvl + span;

    // Set bounds in proper order to satisfy the start < end invariant
    if start >= g.wvl_end() {
        let _ = g.set_wvl_end(end);
        let _ = g.set_wvl_start(start);
    } else {
        let _ = g.set_wvl_start(start);
        let _ = g.set_wvl_end(end);
    }
    let _ = g.set_mu(default_wvl);
    g
}

/// Creates a `LaserLines` distribution containing exactly one line at `default_wvl`.
pub fn default_ray_laser_lines(default_wvl: Length) -> LaserLines {
    // LaserLines::new replaces the default lines directly without violating AllNotEmpty
    LaserLines::new(vec![(default_wvl, 1.0)]).unwrap_or_default()
}

/// Creates an `EnergyLaserLines` distribution containing exactly one line at `default_wvl` with 1.0 J.
pub fn default_energy_laser_lines(default_wvl: Length) -> EnergyLaserLines {
    let resolution = *EnergyLaserLines::default().spectral_resolution();
    // EnergyLaserLines::new sets the line atomically, avoiding failed deletion on single lines
    EnergyLaserLines::new(vec![(default_wvl, joule!(1.0))], resolution).unwrap_or_default()
}

/// Creates a default `EnergyDataBuilder` configured with the application default wavelength.
pub fn default_energy_data_builder(default_wvl: Length) -> EnergyDataBuilder {
    EnergyDataBuilder::LaserLines(default_energy_laser_lines(default_wvl))
}

/// Applies `default_wvl` to the active spectral distribution of a `RayDataSource`.
pub fn apply_default_wavelength_to_ray_source(rds: &mut RayDataSource, default_wvl: Length) {
    match rds {
        RayDataSource::Collimated(_) | RayDataSource::PointSrc(_) => {
            let current = rds.get_spectral_distribution_type().unwrap_or_default();
            let updated = match current {
                SpecDistType::Gaussian(_) => SpecDistType::Gaussian(default_gaussian(default_wvl)),
                SpecDistType::LaserLines(_) => {
                    SpecDistType::LaserLines(default_ray_laser_lines(default_wvl))
                }
            };
            rds.set_spectral_dist(updated);
        }
        RayDataSource::Image(img) => {
            let _ = img.set_wavelength(default_wvl);
        }
        RayDataSource::Raw(_) => {}
    }
}

/// Creates a default `RayDataSource` configured with the application default wavelength.
pub fn default_ray_data_source(default_wvl: Length) -> RayDataSource {
    let mut rds = RayDataSource::default();
    apply_default_wavelength_to_ray_source(&mut rds, default_wvl);
    rds
}