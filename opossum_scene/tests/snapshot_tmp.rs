//! TEMPORARY, never to be committed: writes what opossum_scene draws for fixed models to
//! `$SNAPSHOT_DIR/<label>/`, so a refactor can be checked byte for byte. The models themselves are
//! saved on the first run and loaded from there afterwards, which keeps their uuids fixed.
#![allow(missing_docs)]

use opossum_core::{
    analyzers::{AnalyzerType, RayTraceConfig},
    apertures::{Bore, CircleShape, OpticShape, Outline},
    core_optics::{NodeAttrExt, OpticNode, OpticNodeExt},
    degree,
    distributions::{
        energy::UniformDist,
        position::Hexapolar,
        spectral::{Gaussian, LaserLines},
    },
    geometry::body::OPTIC_SHAPE,
    joule,
    light::lightdata::{
        ray_data_builder::RayDataBuilder,
        ray_data_source::{CollimatedSrc, RayDataSource},
    },
    millimeter, nanometer,
    nodes::{
        BeamSplitter, BoredMirror, CylindricLens, EnergyMeter, IdealFilter, Lens, NodeGroup,
        NodeReference, ParaxialSurface, ReflectiveGrating, SourcePort, SpotDiagram, ThinMirror,
        Wedge,
    },
    opm_document::OpmDocument,
    types::api_types::SceneManifest,
};
use opossum_scene::{AXIS_COLOR, LineColors, glb_of_node, manifest_of, rays_glb, scene_glb};
use std::path::{Path, PathBuf};

fn source(rings: u8, lines: &[f64]) -> RayDataBuilder {
    RayDataSource::Collimated(CollimatedSrc::new(
        Hexapolar::new(millimeter!(2.0), rings).unwrap().into(),
        UniformDist::new(joule!(1.0)).unwrap().into(),
        LaserLines::new(lines.iter().map(|nm| (nanometer!(*nm), 1.0)).collect())
            .unwrap()
            .into(),
    ))
    .into()
}

/// Wedge, cylinder, lens, mirror, the lens again backwards, a detector.
fn chain() -> OpmDocument {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(SourcePort::default()).unwrap();
    let wedge = scenery.add_node(Wedge::default()).unwrap();
    let cylinder = scenery.add_node(CylindricLens::default()).unwrap();
    let lens = scenery.add_node(Lens::default()).unwrap();
    let mirror = scenery.add_node(ThinMirror::default()).unwrap();
    let mut again = NodeReference::from_node(&scenery.node_recursive(lens).unwrap().0).unwrap();
    again.set_inverted(true).unwrap();
    let again = scenery.add_node(again).unwrap();
    let detector = scenery.add_node(EnergyMeter::default()).unwrap();
    let hop = millimeter!(50.0);
    scenery
        .connect_nodes(src, "output_1", wedge, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(wedge, "output_1", cylinder, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(cylinder, "output_1", lens, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(lens, "output_1", mirror, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(mirror, "output_1", again, "output_1", hop)
        .unwrap();
    scenery
        .connect_nodes(again, "input_1", detector, "input_1", hop)
        .unwrap();
    let mut document = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(src, source(3, &[1053.0]));
    document.add_analyzer(AnalyzerType::RayTrace(config));
    document
}

/// A bored mirror with a tilted bore, one with a bore on the axis, a splitter, a filter, a
/// paraxial surface, a grating and a spot diagram, lit by three laser lines.
fn bores_and_surfaces() -> OpmDocument {
    let bored = |tilt: f64| {
        let ring = OpticShape::Bounded(
            Outline::new(
                CircleShape::new(millimeter!(12.5)).unwrap(),
                Some(
                    Bore::new(
                        CircleShape::new(millimeter!(3.0)).unwrap(),
                        millimeter!(4.0, 1.0),
                        degree!(0.0),
                    )
                    .with_tilt(degree!(tilt), degree!(tilt / 2.0)),
                ),
            )
            .unwrap(),
        );
        let mut mirror = BoredMirror::default();
        mirror.set_property(OPTIC_SHAPE, ring.into()).unwrap();
        mirror
    };
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(SourcePort::default()).unwrap();
    let tilted = scenery.add_node(bored(10.0)).unwrap();
    let straight = scenery.add_node(bored(0.0)).unwrap();
    let mut splitter = BeamSplitter::default();
    splitter
        .set_alignment(millimeter!(0., 0., 0.), degree!(0., 45., 0.))
        .unwrap();
    let splitter = scenery.add_node(splitter).unwrap();
    let filter = scenery.add_node(IdealFilter::default()).unwrap();
    let paraxial = scenery.add_node(ParaxialSurface::default()).unwrap();
    let grating = scenery
        .add_node(
            ReflectiveGrating::default()
                .with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))
                .unwrap(),
        )
        .unwrap();
    let spot = scenery.add_node(SpotDiagram::default()).unwrap();
    let hop = millimeter!(50.0);
    scenery
        .connect_nodes(src, "output_1", tilted, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(tilted, "out1_trans1_refl2", straight, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(straight, "out1_trans1_refl2", splitter, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(splitter, "out1_trans1_refl2", filter, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(splitter, "out2_trans2_refl1", paraxial, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(filter, "output_1", grating, "input_1", hop)
        .unwrap();
    scenery
        .connect_nodes(grating, "output_1", spot, "input_1", hop)
        .unwrap();
    let mut document = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    config.map_source(src, source(4, &[990.0, 1000.0, 1010.0]));
    document.add_analyzer(AnalyzerType::RayTrace(config));
    document
}

/// A Gaussian spectrum through a lens.
fn spectrum() -> OpmDocument {
    let mut scenery = NodeGroup::default();
    let src = scenery.add_node(SourcePort::default()).unwrap();
    let lens = scenery.add_node(Lens::default()).unwrap();
    scenery
        .connect_nodes(src, "output_1", lens, "input_1", millimeter!(50.0))
        .unwrap();
    let mut document = OpmDocument::new(scenery);
    let mut config = RayTraceConfig::default();
    let gaussian = Gaussian::new(
        (nanometer!(1003.0), nanometer!(1103.0)),
        20,
        nanometer!(1053.0),
        nanometer!(20.0),
        1.0,
    )
    .unwrap();
    let mut builder = source(3, &[1053.0]);
    let mut src_data = builder.source().clone();
    src_data.set_spectral_dist(gaussian.into());
    builder.set_source(src_data);
    config.map_source(src, builder);
    document.add_analyzer(AnalyzerType::RayTrace(config));
    document
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap();
}

fn snapshot(label: &str, build: fn() -> OpmDocument) {
    let root = PathBuf::from(std::env::var("SNAPSHOT_DIR").expect("SNAPSHOT_DIR"));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    let model_file = models.join(format!("{label}.opm"));
    if !model_file.exists() {
        std::fs::write(&model_file, build().to_opm_file_string().unwrap()).unwrap();
    }
    let document =
        OpmDocument::from_string(&std::fs::read_to_string(&model_file).unwrap()).unwrap();
    let out = root
        .join(std::env::var("SNAPSHOT_LABEL").expect("SNAPSHOT_LABEL"))
        .join(label);
    std::fs::create_dir_all(&out).unwrap();

    let placed = document.placed_scenery(None).unwrap();
    write(&out, "scene.glb", &scene_glb(&placed).unwrap());
    let manifest: SceneManifest = manifest_of(&placed).unwrap();
    write(
        &out,
        "manifest.json",
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );
    for entry in &manifest.nodes {
        let node = document.scenery().node_recursive(entry.uid).unwrap().0;
        let glb = glb_of_node(&*node).unwrap().unwrap();
        write(&out, &format!("node-{}.glb", entry.uid), &glb);
    }
    let axes = document.positioning_rays(None).unwrap();
    write(
        &out,
        "axis.glb",
        &rays_glb(&axes, &LineColors::Fixed(AXIS_COLOR), usize::MAX).unwrap(),
    );
    let range = document.source_wavelength_range(None).unwrap();
    let rays = document.traced_rays(None, 100).unwrap();
    write(
        &out,
        "rays.glb",
        &rays_glb(&rays, &LineColors::Wavelength(range), 100).unwrap(),
    );
}

#[test]
fn snapshot_all() {
    snapshot("chain", chain);
    snapshot("bores_and_surfaces", bores_and_surfaces);
    snapshot("spectrum", spectrum);
}
