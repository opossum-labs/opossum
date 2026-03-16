use opossum_core::coatings::CoatingType;
use opossum_core::prelude::*;
use opossum_core::{
    energy_distributions::General2DGaussian, position_distributions::HexagonalTiling,
    spectral_distribution::LaserLines,
};
use core::f64;
use std::path::Path;

fn main() -> OpmResult<()> {
    let mut scenery = NodeGroup::new("Ghost focus nested group test");
    scenery.set_expand_view(true)?;

    let inf = millimeter!(f64::INFINITY);

    let mut lens_01 = Lens::new("Lens 0_1", inf, inf, millimeter!(1.), RefrIndexConst::new(1.4)?)?;
    lens_01.set_coating(&PortType::Input, "input_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    lens_01.set_coating(&PortType::Output, "output_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;

    let mut lens_02 = Lens::new("Lens 0_2", inf, inf, millimeter!(1.), RefrIndexConst::new(1.4)?)?;
    lens_02.set_coating(&PortType::Input, "input_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    lens_02.set_coating(&PortType::Output, "output_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;

    let src = scenery.add_node(SourcePort::new("Collimated Source"))?;
    let l0_1 = scenery.add_node(lens_01)?;
    let l0_2 = scenery.add_node(lens_02)?;

    let mut lens_1 = Lens::new("Lens 1", inf, inf, millimeter!(1.), RefrIndexConst::new(1.4)?)?;
    lens_1.set_coating(&PortType::Input, "input_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    lens_1.set_coating(&PortType::Output, "output_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;

    let mut group_1 = NodeGroup::new("Group 1");
    group_1.set_expand_view(true)?;
    let l1 = group_1.add_node(lens_1)?;
    
    let mut lens_2 = Lens::new("Lens 2", inf, inf, millimeter!(1.), RefrIndexConst::new(1.4)?)?;
    lens_2.set_coating(&PortType::Input, "input_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    lens_2.set_coating(&PortType::Output, "output_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    let mut group_2 = NodeGroup::new("Group 2");
    group_2.set_expand_view(true)?;
    let l2 = group_2.add_node(lens_2)?;
    
    let mut lens_3 = Lens::new("Lens 3", inf, inf, millimeter!(1.), RefrIndexConst::new(1.4)?)?;
    lens_3.set_coating(&PortType::Input, "input_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    lens_3.set_coating(&PortType::Output, "output_1", &CoatingType::ConstantR { reflectivity: 0.01 })?;
    let mut group_3 = NodeGroup::new("Group 3");
    group_3.set_expand_view(true)?;
    let l3 = group_3.add_node(lens_3)?;
    group_3.map_input_port(l3, "input_1", "input_1")?;
    group_3.map_output_port(l3, "output_1", "output_1")?;      
    
    
    let g3 = group_2.add_node(group_3)?;
    group_2.connect_nodes(l2, "output_1", g3, "input_1", millimeter!(10.))?;
    group_2.map_input_port(l2, "input_1", "input_1")?;
    group_2.map_output_port(g3, "output_1", "output_1")?;  
    
    let g2 = group_1.add_node(group_2)?;
    group_1.connect_nodes(l1, "output_1", g2, "input_1", millimeter!(10.))?;
    group_1.map_input_port(l1, "input_1", "input_1")?;
    group_1.map_output_port(g2, "output_1", "output_1")?;    
    
    let g1 = scenery.add_node(group_1)?;
    

    scenery.connect_nodes(src, "output_1", l0_1, "input_1", millimeter!(10.))?;
    scenery.connect_nodes(l0_1, "output_1", g1, "input_1", millimeter!(10.))?;
    scenery.connect_nodes(g1, "output_1", l0_2, "input_1", millimeter!(10.))?;
    
    // collimated source definition
    let ray_data_source = RayDataSource::Collimated(CollimatedSrc::new(
        HexagonalTiling::new(millimeter!(10.), 1, millimeter!(0., 0.))?.into(),
        General2DGaussian::new(
            joule!(5.0),
            millimeter!(0., 0.),
            millimeter!(2., 2.),
            5.,
            radian!(0.),
            false,
        )?
        .into(),
        LaserLines::new(vec![(nanometer!(1053.0), 1.0)])?.into(),
    ));

    let mut doc = OpmDocument::new(scenery);

    //analyzers are added in the tests
    doc.save_to_file(Path::new("./opossum_core/playground/ghost_focus_nested_group_test.opm"))
}
