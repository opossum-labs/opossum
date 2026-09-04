use nalgebra::Point2;
use num_traits::Zero;
use opossum_core::{
    distributions::energy::{EnergyDistType, UniformDist},
    distributions::position::{
        FibonacciRectangle, Grid, HexagonalTiling, Hexapolar, PosDistType, Random, SobolDist,
    },
    distributions::spectral::{LaserLines, SpecDistType},
    light::lightdata::ray_data_builder::RayDataBuilder,
    prelude::*,
};
use std::path::Path;
use uom::si::f64::Length;
use uuid::Uuid;
fn main() -> OpmResult<()> {
    let energy_dist: EnergyDistType = UniformDist::new(joule!(1.0))?.into();
    let spec_dist: SpecDistType = LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?.into();

    let mut scenery = NodeGroup::new("Ray distribution demo");
    let src_hexapolar = scenery.add_node(SourcePort::new("hexapolar"))?;
    let src_hexagonal = scenery.add_node(SourcePort::new("hexagonal"))?;
    let src_grid = scenery.add_node(SourcePort::new("grid"))?;
    let src_fibonacci = scenery.add_node(SourcePort::new("fibonacci"))?;
    let src_sobol = scenery.add_node(SourcePort::new("sobol"))?;
    let src_random = scenery.add_node(SourcePort::new("random"))?;

    let i_bs = scenery.add_node(BeamSplitter::default())?;
    let i_bs2 = scenery.add_node(BeamSplitter::default())?;
    let i_bs3 = scenery.add_node(BeamSplitter::default())?;
    let i_bs4 = scenery.add_node(BeamSplitter::default())?;
    let i_bs5 = scenery.add_node(BeamSplitter::default())?;

    let i_sd = scenery.add_node(SpotDiagram::default())?;

    scenery.connect_nodes(src_hexapolar, "output_1", i_bs, "input_1", Length::zero())?;
    scenery.connect_nodes(src_hexagonal, "output_1", i_bs, "input_2", Length::zero())?;

    scenery.connect_nodes(src_grid, "output_1", i_bs2, "input_1", Length::zero())?;
    scenery.connect_nodes(src_fibonacci, "output_1", i_bs3, "input_1", Length::zero())?;
    scenery.connect_nodes(src_sobol, "output_1", i_bs4, "input_1", Length::zero())?;
    scenery.connect_nodes(src_random, "output_1", i_bs5, "input_1", Length::zero())?;

    scenery.connect_nodes(i_bs, "out1_trans1_refl2", i_bs2, "input_2", Length::zero())?;
    scenery.connect_nodes(i_bs2, "out1_trans1_refl2", i_bs3, "input_2", Length::zero())?;
    scenery.connect_nodes(i_bs3, "out1_trans1_refl2", i_bs4, "input_2", Length::zero())?;
    scenery.connect_nodes(i_bs4, "out1_trans1_refl2", i_bs5, "input_2", Length::zero())?;

    scenery.connect_nodes(i_bs5, "out1_trans1_refl2", i_sd, "input_1", Length::zero())?;

    let mut doc = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();

    let builders: Vec<(Uuid, PosDistType, Isometry)> = vec![
        (
            src_hexapolar,
            Hexapolar::new(millimeter!(5.0), 3)?.into(),
            Isometry::identity(),
        ),
        (
            src_hexagonal,
            HexagonalTiling::new(millimeter!(5.0), 3)?.into(),
            Isometry::new_translation(millimeter!(-20.0, 0.0, 0.0))?,
        ),
        (
            src_grid,
            Grid::new(millimeter!(10.0, 10.0), Point2::new(7, 7))?.into(),
            Isometry::new_translation(millimeter!(20.0, 0.0, 0.0))?,
        ),
        (
            src_fibonacci,
            FibonacciRectangle::new(millimeter!(10.0), millimeter!(10.0), 50)?.into(),
            Isometry::new_translation(millimeter!(-20.0, 20.0, 0.0))?,
        ),
        (
            src_sobol,
            SobolDist::new(millimeter!(10.0), millimeter!(10.0), 60)?.into(),
            Isometry::new_translation(millimeter!(0.0, 20.0, 0.0))?,
        ),
        (
            src_random,
            Random::new(millimeter!(5.0), millimeter!(5.0), 60)?.into(),
            Isometry::new_translation(millimeter!(20.0, 20.0, 0.0))?,
        ),
    ];
    for (src, pos_dist, iso) in builders {
        let collimated_src = CollimatedSrc::new(pos_dist, energy_dist, spec_dist.clone());
        let ray_data_source = RayDataSource::Collimated(collimated_src);
        let mut ray_data_builder: RayDataBuilder = ray_data_source.into();
        ray_data_builder.set_isometry(Some(iso));
        config.map_source(src, ray_data_builder);
    }
    doc.add_analyzer(AnalyzerType::RayTrace(config));
    doc.save_to_file(Path::new("./opossum_core/playground/ray_source.opm"))
}
