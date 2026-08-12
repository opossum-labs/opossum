use opossum_core::asset::AssetHeader;
use opossum_core::material::Material;
use uom::si::f64::Length;
use uom::si::length::nanometer;

use crate::asset::RegisterableAsset;
use crate::index::{IndexableAsset, MaterialIndexData, WAVELENGTH_D_LINE_NM};

impl RegisterableAsset for Material {
    fn header(&self) -> &AssetHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut AssetHeader {
        &mut self.header
    }
    fn relative_subfolder() -> &'static str {
        "materials"
    }
}

impl IndexableAsset for Material {
    type IndexData = MaterialIndexData;

    fn create_index_data(&self) -> Self::IndexData {
        let d_line_wvl = Length::new::<nanometer>(WAVELENGTH_D_LINE_NM);
        let nd = self.get_refractive_index(d_line_wvl).ok();

        MaterialIndexData { nd }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::refractive_index::RefrIndexConst;

    #[test]
    fn test_core_material_registry_integration() {
        let const_refr = RefrIndexConst::new(1.5).unwrap().into();

        let material = Material::new_draft(
            "N-BK7",
            Some("Schott".to_string()),
            Some("Standard Crown Glass".to_string()),
            const_refr,
        );
        assert_eq!(material.version(), 0);
        assert_eq!(material.name(), "N-BK7");
        assert_eq!(Material::relative_subfolder(), "materials");
    }
}
