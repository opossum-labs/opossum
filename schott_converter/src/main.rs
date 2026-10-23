use std::collections::HashMap;
use std::error::Error;
use std::{fs, path::Path};

use csv::ReaderBuilder;
use uuid::Uuid;

use uom::si::{
    f64::{Length, MassDensity, Pressure, TemperatureCoefficient, ThermalConductivity},
    length::{millimeter, nanometer},
    mass_density::kilogram_per_cubic_meter,
    pressure::pascal,
    temperature_coefficient::per_kelvin,
    thermal_conductivity::watt_per_meter_kelvin,
};

use opossum_core::{
    absorption::absorption_model::AbsorptionModel,
    light::Spectrum,
    material::{Material, MechanicalProperties, ThermalProperties},
    refractive_index::{RefrIndexSellmeier1, RefractiveIndexType},
};

fn main() -> Result<(), Box<dyn Error>> {
    let input_path = "schott-optical-glass.csv";
    let output_dir = "catalogs/materials";

    println!("Starting conversion from {input_path}");
    let materials = parse_schott_csv(input_path)?;
    println!("Successfully parsed {} glasses.", materials.len());

    // Create the output directory if it does not exist
    fs::create_dir_all(output_dir)?;

    let mut success_count = 0;
    for mut material in materials {
        // Assign a unique ID to every newly generated material
        let id = Uuid::new_v4();
        material.header.id = id;
        material.header.version = 1;

        let mat_dir = format!("{output_dir}/{id}");
        fs::create_dir_all(&mat_dir)?;

        let file_path = format!("{mat_dir}/v1.ron");
        let ron_string = ron::ser::to_string_pretty(&material, ron::ser::PrettyConfig::default())?;

        fs::write(&file_path, ron_string)?;
        success_count += 1;
    }
    println!("Exported {success_count} materials to RON files in '{output_dir}'.");
    Ok(())
}

/// Parses the SCHOTT CSV file and maps it directly into OPOSSUM `Material` structs.
/// Handled memory-safe by replacing invalid UTF-8 characters lossily.
fn parse_schott_csv<P: AsRef<Path>>(path: P) -> Result<Vec<Material>, Box<dyn Error>> {
    // 1. Read the raw bytes of the file to bypass strict UTF-8 validation
    let raw_bytes = fs::read(path)?;

    // 2. Convert to a UTF-8 String, replacing invalid characters with the replacement character
    let lossy_content = String::from_utf8_lossy(&raw_bytes).into_owned();

    // 3. Parse the cleaned string content using the csv crate
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(false) // We handle headers manually due to the file structure
        .from_reader(lossy_content.as_bytes());

    let mut iter = reader.records();

    // Skip the first 3 lines containing metadata and overarching categories
    for _ in 0..3 {
        iter.next();
    }

    // Row 3 (0-indexed) contains the actual property names
    let headers_record = iter.next().ok_or("Missing header row")??;
    let mut header_map = HashMap::new();
    for (i, h) in headers_record.iter().enumerate() {
        header_map.insert(h.trim().to_string(), i);
    }

    let mut materials = Vec::new();

    for record_result in iter {
        let record = record_result?;

        // Helper closure to extract string data by column name
        let get_str = |col: &str| -> Option<&str> {
            header_map
                .get(col)
                .and_then(|&idx| record.get(idx))
                .map(str::trim)
        };

        // Helper closure to extract and parse f64 data by column name
        let get_f64 =
            |col: &str| -> Option<f64> { get_str(col).and_then(|s| s.parse::<f64>().ok()) };

        let Some(name) = get_str("Glass") else {
            continue;
        };
        if name.is_empty() {
            continue; // Skip empty rows
        }

        // --- Optical Properties (Refractive Index) ---
        // Require all Sellmeier coefficients. If missing, we skip this glass.
        let k1 = get_f64("B1");
        let k2 = get_f64("B2");
        let k3 = get_f64("B3");
        let l1 = get_f64("C1");
        let l2 = get_f64("C2");
        let l3 = get_f64("C3");

        let (Some(k1), Some(k2), Some(k3), Some(l1), Some(l2), Some(l3)) = (k1, k2, k3, l1, l2, l3)
        else {
            println!("Skipping {name} due to missing Sellmeier coefficients.");
            continue;
        };

        // Standard validity range for optical catalog glasses
        let valid_range = Length::new::<nanometer>(300.0)..Length::new::<nanometer>(2500.0);
        let refr_model = RefrIndexSellmeier1::new(k1, k2, k3, l1, l2, l3, valid_range)?;

        let mut material = Material::new_draft(
            name,
            Some("Schott".to_string()),
            Some("Optical glass imported from Schott catalog".to_string()),
            RefractiveIndexType::Sellmeier1(refr_model),
        );

        // --- Optical Properties (Absorption) ---
        let mut transmissions = Vec::new();
        for (header_name, &idx) in &header_map {
            // Find all columns that represent transmission data at 10mm thickness
            if let Some(lambda_str) = header_name.strip_prefix("TAUI10/")
                && let Ok(wavelength_nm) = lambda_str.parse::<f64>()
                && let Some(cell) = record.get(idx)
                && let Ok(tau) = cell.trim().parse::<f64>()
            {
                // OPOSSUM Spectrum uses wavelength in micrometers
                transmissions.push((wavelength_nm * 1e-3, tau));
            }
        }

        // The Spectrum struct strictly requires monotonically increasing wavelengths (XNormal validator)
        transmissions.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        if !transmissions.is_empty() {
            let mut spectrum = Spectrum::default();
            spectrum.set_data(transmissions)?;

            // TAUI10 always uses a reference thickness of 10 mm
            let abs_model = AbsorptionModel::new_catalog_transmittance(
                Length::new::<millimeter>(10.0),
                spectrum,
            )?;
            material.optical.absorption = abs_model;
        }

        // --- Thermal Properties ---
        let t_cond = get_f64("Heat conductivity(lambda)")
            .map(|v| ThermalConductivity::new::<watt_per_meter_kelvin>(v));

        // Convert from 1e-6/K to 1/K for the expansion coefficient
        let t_exp =
            get_f64("alpha -30/70").map(|v| TemperatureCoefficient::new::<per_kelvin>(v * 1e-6));

        if t_cond.is_some() || t_exp.is_some() {
            material.thermal = Some(ThermalProperties::new(t_cond, t_exp));
        }

        // --- Mechanical Properties ---
        // Density in SCHOTT is usually in g/cm^3. uom base unit is kg/m^3.
        let density =
            get_f64("Density").map(|v| MassDensity::new::<kilogram_per_cubic_meter>(v * 1000.0));

        // The Schott CSV uses an acute accent "´" instead of a regular apostrophe in some versions.
        // We provide a fallback just in case the format normalizes in the future.
        // The unit is GPa, so we multiply by 1e9 to get Pascals.
        let youngs_modulus = get_f64("Young´s modulus (E)")
            .or_else(|| get_f64("Young's modulus (E)"))
            .map(|v| Pressure::new::<pascal>(v * 1e9));

        // Only create the MechanicalProperties struct if at least one field is valid
        if density.is_some() || youngs_modulus.is_some() {
            material.mechanical = Some(MechanicalProperties::new(density, youngs_modulus));
        }

        materials.push(material);
    }

    Ok(materials)
}
