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
    use opossum_core::refractive_index::{RefrIndexConst, RefractiveIndexType};
    use uuid::Uuid;

    #[test]
    fn test_core_material_registry_integration() {
        let id = Uuid::new_v4();
        let const_refr = RefractiveIndexType::Const(RefrIndexConst::new(1.5).unwrap());

        let material = Material::new(
            id,
            1,
            "N-BK7",
            Some("Schott".to_string()),
            Some("Standard Crown Glass".to_string()),
            const_refr,
        );

        assert_eq!(material.id(), id);
        assert_eq!(material.version(), 1);
        assert_eq!(material.name(), "N-BK7");
        assert_eq!(Material::relative_subfolder(), "materials");
    }
}
