//! Serialization, deserialization, and export capabilities for [`OpmDocument`].

use super::OpmDocument;
use crate::{
    core_optics::OpticNode,
    error::{OpmResult, OpossumError},
    properties::{Proptype, proptype::AssetRef},
    utils::file_utils::{create_f_path, create_file_instance},
};
use log::warn;
use ron::{extensions::Extensions, ser::PrettyConfig};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

impl OpmDocument {
    /// Replaces all `AssetRef::Id(Uuid)` properties in scene nodes (including all nested
    /// groups) with the full `AssetRef::Inline(Material)` looked up from `embedded_materials`.
    pub(crate) fn resolve_embedded_materials(&mut self) -> OpmResult<()> {
        let embedded_materials = &self.embedded_materials;
        let mut err = None;
        self.scenery.for_each_node_mut(&mut |node_ref| {
            if err.is_some() {
                return;
            }
            let mut updates = Vec::new();
            for (prop_name, prop) in node_ref.node_attr().properties() {
                if let Proptype::Material(AssetRef::Id(id)) = prop.prop() {
                    let Some(material) = embedded_materials.get(id) else {
                        err = Some(OpossumError::OpmDocument(format!(
                            "Embedded material with UUID {id} not found for property '{prop_name}' in node '{}'",
                            node_ref.node_attr().name()
                        )));
                        return;
                    };
                    updates.push((
                        prop_name.clone(),
                        Proptype::Material(AssetRef::Inline(material.clone())),
                    ));
                }
            }
            for (prop_name, new_prop) in updates {
                if let Err(e) = node_ref.node_attr_mut().set_property(&prop_name, new_prop) {
                    err = Some(e);
                    return;
                }
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        Ok(())
    }

    /// Extracts full `Material` structs into `embedded_materials` from all nodes
    /// (including all nested groups) and replaces node properties with explicit `AssetRef::Id(Uuid)`.
    pub(crate) fn prepare_materials_for_serialization(&mut self) -> OpmResult<()> {
        let embedded_materials = &mut self.embedded_materials;
        let mut err = None;
        self.scenery.for_each_node_mut(&mut |node_ref| {
            if err.is_some() {
                return;
            }
            let mut updates = Vec::new();
            for (prop_name, prop) in node_ref.node_attr().properties() {
                if let Proptype::Material(AssetRef::Inline(material)) = prop.prop() {
                    embedded_materials.insert(material.id(), material.clone());
                    updates.push((
                        prop_name.clone(),
                        Proptype::Material(AssetRef::Id(material.id())),
                    ));
                }
            }
            for (prop_name, new_prop) in updates {
                if let Err(e) = node_ref.node_attr_mut().set_property(&prop_name, new_prop) {
                    err = Some(e);
                    return;
                }
            }
        });
        if let Some(e) = err {
            return Err(e);
        }
        Ok(())
    }

    /// Creates a new [`OpmDocument`] from an `.opm` file at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be read or parsing fails.
    pub fn from_file(path: &Path) -> OpmResult<Self> {
        let contents = fs::read_to_string(path).map_err(|e| {
            OpossumError::OpmDocument(format!("cannot read file {} : {}", path.display(), e))
        })?;
        Self::from_string(&contents)
    }

    /// Creates a new [`OpmDocument`] from the given `.opm` file string content.
    ///
    /// # Errors
    ///
    /// Returns an error if the RON deserialization or graph hook resolution fails.
    pub fn from_string(file_string: &str) -> OpmResult<Self> {
        let mut document: Self = ron::from_str(file_string)
            .map_err(|e| OpossumError::OpmDocument(format!("parsing of model failed: {e}")))?;

        if document.opm_file_version != env!("OPM_FILE_VERSION") {
            warn!("OPM file version does not match the used OPOSSUM version.");
            warn!(
                "read version '{}' <-> program file version '{}'",
                document.opm_file_version,
                env!("OPM_FILE_VERSION")
            );
            warn!(
                "This file might have been written by an older or newer version of OPOSSUM. The model import might not be correct."
            );
        }

        document.scenery.after_deserialization_hook()?;
        // Resolve cross-group node references across the entire graph
        document.scenery.graph_mut().resolve_all_references()?;

        // Resolve embedded material references into full in-memory Material structs
        document.resolve_embedded_materials()?;
        Ok(document)
    }

    /// Saves this [`OpmDocument`] to an `.opm` file at the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if file creation, writing, or serialization fails.
    pub fn save_to_file(&self, path: &Path) -> OpmResult<()> {
        let serialized = self.to_opm_file_string()?;
        let mut output = File::create(path).map_err(|e| {
            OpossumError::OpticScenery(format!(
                "could not create file path {}: {}",
                path.display(),
                e
            ))
        })?;
        write!(output, "{serialized}").map_err(|e| {
            OpossumError::OpticScenery(format!(
                "writing to file path {} failed: {}",
                path.display(),
                e
            ))
        })?;
        Ok(())
    }

    /// Generates the RON string representation of this [`OpmDocument`].
    ///
    /// Extracts embedded materials and replaces node material properties with UUID references.
    /// Operates on an isolated clone to keep `&self` unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if material extraction or RON formatting fails.
    pub fn to_opm_file_string(&self) -> OpmResult<String> {
        let mut doc_to_serialize = self.clone();
        doc_to_serialize.prepare_materials_for_serialization()?;

        let config = PrettyConfig::new()
            .extensions(Extensions::UNWRAP_VARIANT_NEWTYPES)
            .new_line("\n");

        ron::ser::to_string_pretty(&doc_to_serialize, config).map_err(|e| {
            OpossumError::OpticScenery(format!("serialization of OpmDocument failed: {e}"))
        })
    }

    /// Creates DOT and SVG diagram files representing the optical scenery.
    ///
    /// # Errors
    ///
    /// Returns an error if generating or writing the diagram files fails.
    pub fn create_dot_file(&self, dot_path: &Path) -> OpmResult<()> {
        let mut output = create_file_instance(dot_path, "scenery", "dot")?;
        write!(output, "{}", self.scenery.toplevel_dot("")?)
            .map_err(|e| OpossumError::Other(format!("writing diagram file (.dot) failed: {e}")))?;
        let mut output = create_file_instance(dot_path, "scenery", "svg")?;
        let f_path = create_f_path(dot_path, "scenery", "dot");
        self.scenery.toplevel_dot_svg(&f_path, &mut output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analyzers::{
            Analyzer, AnalyzerType, GhostFocusConfig, RayTraceConfig,
            ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer,
        },
        core_optics::{Alignable, OpticNode, PortType, node_attr::NodePositioning},
        degree,
        gain::{ConstGain, GainModel},
        joule,
        material::{MATERIAL, Material},
        millimeter, nanometer,
        nodes::{
            BeamSplitter, CylindricLens, Dummy, EnergyMeter, FluenceDetector, IdealFilter, Lens,
            NodeGroup, ParabolicMirror, ParaxialSurface, RayPropagationVisualizer,
            ReflectiveGrating, SourcePort, Spectrometer, SpotDiagram, ThinMirror, WaveFront, Wedge,
            collimated_line_ray_builder, round_collimated_ray_builder,
        },
        refractive_index::RefrIndexConst,
        utils::test_helper::test_helper::check_logs,
    };
    use std::path::PathBuf;
    use tempfile::NamedTempFile;
    use uuid::Uuid;

    #[test]
    fn pump_scenarios_survive_a_file_round_trip() -> OpmResult<()> {
        let mut document = OpmDocument::default();
        let lens_id = document.scenery_mut().add_node(Lens::default())?;
        let scenario_id = document.add_pump_scenario("full power");
        let gain = GainModel::Const(ConstGain::new(2.0)?);
        document
            .pump_scenario_mut(scenario_id)
            .expect("scenario just added must exist")
            .set_gain_model(lens_id, gain);

        let serialized = document.to_opm_file_string()?;
        let reloaded = OpmDocument::from_string(&serialized)?;
        assert_eq!(
            reloaded
                .pump_scenario(scenario_id)
                .map(|scenario| scenario.gain_model(lens_id)),
            Some(gain)
        );
        Ok(())
    }

    #[test]
    fn a_document_without_scenarios_writes_none() -> OpmResult<()> {
        let document = OpmDocument::default();
        assert!(!document.to_opm_file_string()?.contains("pump_scenarios"));
        Ok(())
    }

    #[test]
    fn amplifier_nodes_survive_a_file_round_trip() -> OpmResult<()> {
        let mut document = OpmDocument::default();
        let lens_id = document.scenery_mut().add_node(Lens::default())?;
        document.set_is_amplifier_node(lens_id, true);

        let serialized = document.to_opm_file_string()?;
        let reloaded = OpmDocument::from_string(&serialized)?;
        assert!(reloaded.is_amplifier_node(lens_id));
        Ok(())
    }

    #[test]
    fn a_document_without_amplifier_nodes_writes_none() -> OpmResult<()> {
        let document = OpmDocument::default();
        assert!(!document.to_opm_file_string()?.contains("amplifier_nodes"));
        Ok(())
    }

    #[test]
    fn from_file() {
        let result =
            OpmDocument::from_file(Path::new("./invalid_file_path/invalid_file.invalid_ext"));
        assert!(result.unwrap_err().to_string().starts_with(
            "OpmDocument:cannot read file ./invalid_file_path/invalid_file.invalid_ext"
        ));
        let result = OpmDocument::from_file(Path::new("./files_for_testing/opm/incorrect_opm.opm"));
        assert_eq!(
            result.unwrap_err().to_string(),
            "OpmDocument:parsing of model failed: 1:2: Unexpected missing field named `opm_file_version` in `OpmDocument`"
        );
        assert!(
            OpmDocument::from_file(&PathBuf::from("./files_for_testing/opm/opticscenery.opm"))
                .is_ok()
        );
    }

    #[test]
    fn reference_into_ancestor_round_trips() {
        use crate::nodes::NodeReference;

        let mut document = OpmDocument::default();
        let r_id = {
            let scenery = document.scenery_mut();
            let a_id = scenery.add_node(Dummy::default()).unwrap();
            let a_ref = scenery.node_recursive(a_id).unwrap().0;
            let mut g = NodeGroup::new("G");
            let r_id = g
                .add_node(NodeReference::from_node(&a_ref).unwrap())
                .unwrap();
            scenery.add_node(g).unwrap();
            r_id
        };

        let serialized = document.to_opm_file_string().unwrap();
        let reloaded = OpmDocument::from_string(&serialized)
            .expect("a reference pointing at an ancestor node must reload");

        let (reference, _) = reloaded
            .scenery()
            .node_recursive(r_id)
            .expect("the reference must still exist after reload");
        let ports = reference.ports();
        assert!(
            !ports.names(&PortType::Output).is_empty(),
            "the reloaded reference must resolve to A (non-empty mirrored ports)"
        );
    }

    #[test]
    fn save_to_file() -> OpmResult<()> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| OpossumError::OpmDocument(format!("Error creating temp dir: {e}")))?;
        let target_path = temp_dir.path().join("document.opm");

        let document = OpmDocument::default();
        document.save_to_file(&target_path)?;
        assert!(target_path.is_file(), "Saved .opm file was not created");
        let file_len = fs::metadata(&target_path)
            .map_err(|e| OpossumError::OpmDocument(format!("Failed to read metadata: {e}")))?
            .len();
        assert!(file_len > 0, "Saved .opm file is empty");
        Ok(())
    }

    #[test]
    fn test_material_referencing_serialization_roundtrip() -> OpmResult<()> {
        let material_id = Uuid::new_v4();
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_for_test(material_id, 1, "N-BK7 Shared", const_refr.into());

        let mut scenery = NodeGroup::default();
        let lens1 = Lens::new(
            "Lens 1",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            material.clone(),
        )?;
        let lens2 = Lens::new(
            "Lens 2",
            millimeter!(200.0),
            millimeter!(-200.0),
            millimeter!(12.0),
            material,
        )?;

        scenery.add_node(lens1)?;
        scenery.add_node(lens2)?;

        let doc = OpmDocument::new(scenery);
        let ron_str = doc.to_opm_file_string()?;

        assert!(ron_str.contains("embedded_materials:"));
        assert!(ron_str.contains("N-BK7 Shared"));
        assert!(ron_str.contains("Id("));

        let reloaded_doc = OpmDocument::from_string(&ron_str)?;
        assert_eq!(reloaded_doc.embedded_materials.len(), 1);

        for node_ref in reloaded_doc.scenery().nodes() {
            let prop = node_ref.node_attr().get_property(MATERIAL)?;
            if let Proptype::Material(AssetRef::Inline(mat)) = prop {
                assert_eq!(mat.id(), material_id);
                assert_eq!(mat.name(), "N-BK7 Shared");
            } else {
                panic!(
                    "Expected Proptype::Material(AssetRef::Inline) in node after deserialization resolution"
                );
            }
        }

        Ok(())
    }

    #[test]
    fn saving_twice_keeps_the_materials() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let lens_id = scenery.add_node(Lens::default())?;
        let document = OpmDocument::new(scenery);

        let first = document.to_opm_file_string()?;
        let second = document.to_opm_file_string()?;
        assert_eq!(
            first, second,
            "writing the same document twice must produce the same file"
        );

        OpmDocument::from_string(&second)?;

        let node = document.scenery().node(lens_id)?;
        assert!(
            matches!(
                node.node_attr().get_property(MATERIAL),
                Ok(Proptype::Material(AssetRef::Inline(_)))
            ),
            "saving must not leave the node holding a bare material reference"
        );
        Ok(())
    }

    #[test]
    fn test_skip_unknown_node_type_during_deserialization() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "84d4007c-514e-44ca-8b63-bfa86bee265f",
        "graph": (
            nodes: [
                {
                    "node_type": "dummy",
                    "name": "valid_dummy_1",
                    "uuid": "26e56527-c7f9-4e9c-9cda-0e0fa96e39bd",
                },
                {
                    "node_type": "unknown_future_node",
                    "name": "invalid_node",
                    "uuid": "c52398ba-1742-4d86-82e1-8e75874d91ba",
                },
                {
                    "node_type": "dummy",
                    "name": "valid_dummy_2",
                    "uuid": "54e2d453-9632-4b9d-b9c7-48491526f198",
                },
            ],
            edges: [],
        ),
    },
    global: (
        ambient_material: {
            "schema_version": 1,
            "id": "6c30ef98-7380-4477-bc91-a5a1a407fec7",
            "version": 0,
            "name": "Custom Material",
            "optical": (
                refractive_index: Const(
                    refractive_index: 1.0,
                ),
                absorption: r#None,
            ),
        },
    ),
)"#;

        testing_logger::setup();
        let doc = OpmDocument::from_string(ron_data)?;

        assert_eq!(
            doc.scenery().nodes().len(),
            2,
            "Document graph should contain exactly 2 valid nodes after skipping the unknown node"
        );

        check_logs(
            log::Level::Warn,
            vec![
                "Skipping node that failed to load (node_type: unknown_future_node, name: invalid_node, uuid: c52398ba-1742-4d86-82e1-8e75874d91ba).",
            ],
        );

        Ok(())
    }

    #[test]
    fn test_skip_invalid_edge_connection() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "0e0e825e-5f4c-4e0a-b6ce-6c25b608f4aa",
        "graph": (
            nodes: [
                {
                    "node_type": "dummy",
                    "name": "dummy_1",
                    "uuid": "ecb719a2-2e21-44d0-b0b9-1e4a813e964e",
                },
            ],
            edges: [
                (
                    src_id: "ecb719a2-2e21-44d0-b0b9-1e4a813e964e",
                    src_port: "output_1",
                    target_id: "00000000-0000-0000-0000-000000000000",
                    target_port: "input_1",
                    distance: 0.0,
                ),
            ],
        ),
    },
    global: (
        ambient_material: {
            "schema_version": 1,
            "id": "6c30ef98-7380-4477-bc91-a5a1a407fec7",
            "version": 0,
            "name": "Custom Material",
            "optical": (
                refractive_index: Const(
                    refractive_index: 1.0,
                ),
                absorption: r#None,
            ),
        },
    ),
)"#;

        testing_logger::setup();
        let doc = OpmDocument::from_string(ron_data)?;

        assert_eq!(
            doc.scenery().nodes().len(),
            1,
            "The valid node should be present in the graph"
        );

        check_logs(
            log::Level::Warn,
            vec![
                "Skipping invalid node connection from 'ecb719a2-2e21-44d0-b0b9-1e4a813e964e' (output_1) to '00000000-0000-0000-0000-000000000000' (input_1): OpticScenery:target node with given id does not exist",
            ],
        );

        Ok(())
    }

    #[test]
    fn test_load_nested_group_with_mapped_ports() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "84d4007c-514e-44ca-8b63-bfa86bee265f",
        "graph": (
            nodes: [
                {
                    "node_type": "group",
                    "name": "nested group",
                    "uuid": "3f7e3f9e-6b1a-4b1a-8c1a-9e6b1a4b1a8c",
                    "graph": (
                        nodes: [
                            {
                                "node_type": "dummy",
                                "name": "d1",
                                "uuid": "26e56527-c7f9-4e9c-9cda-0e0fa96e39bd",
                            },
                        ],
                        edges: [],
                        input_map: ({
                            "input_1": ("26e56527-c7f9-4e9c-9cda-0e0fa96e39bd", "input_1"),
                        }),
                        output_map: ({
                            "output_1": ("26e56527-c7f9-4e9c-9cda-0e0fa96e39bd", "output_1"),
                        }),
                    ),
                },
            ],
            edges: [],
        ),
    },
    global: (
        ambient_material: {
            "schema_version": 1,
            "id": "6c30ef98-7380-4477-bc91-a5a1a407fec7",
            "version": 0,
            "name": "Custom Material",
            "optical": (
                refractive_index: Const(
                    refractive_index: 1.0,
                ),
                absorption: r#None,
            ),
        },
    ),
)"#;

        testing_logger::setup();
        let doc = OpmDocument::from_string(ron_data)?;

        assert_eq!(
            doc.scenery().nodes().len(),
            1,
            "the nested group must survive loading"
        );

        let (input_names, output_names) = {
            let group_ref = &doc.scenery().nodes()[0];
            let group = group_ref
                .as_any()
                .downcast_ref::<NodeGroup>()
                .expect("the surviving node must still be the nested group");
            (
                group.graph().port_map(&PortType::Input).port_names(),
                group.graph().port_map(&PortType::Output).port_names(),
            )
        };
        assert_eq!(input_names, vec!["input_1".to_string()]);
        assert_eq!(output_names, vec!["output_1".to_string()]);
        check_logs(log::Level::Warn, vec![]);

        Ok(())
    }

    #[test]
    fn create_dot_file_test() -> OpmResult<()> {
        let document =
            OpmDocument::from_file(Path::new("./files_for_testing/opm/opticscenery.opm"))?;
        let temp_dir = tempfile::tempdir()
            .map_err(|e| OpossumError::OpmDocument(format!("Error creating temp dir: {e}")))?;
        let non_existent_path = temp_dir.path().join("non_existent_folder");
        assert!(
            document.create_dot_file(&non_existent_path).is_err(),
            "Writing to an invalid/non-existent directory should return an error"
        );
        assert!(
            document.create_dot_file(temp_dir.path()).is_ok(),
            "Writing dot and svg files to valid directory failed"
        );
        let dot_file = temp_dir.path().join("scenery.dot");
        let svg_file = temp_dir.path().join("scenery.svg");

        assert!(dot_file.is_file(), "scenery.dot was not created");
        assert!(svg_file.is_file(), "scenery.svg was not created");

        let dot_len = fs::metadata(&dot_file)
            .map_err(|e| OpossumError::OpmDocument(format!("Failed to read dot metadata: {e}")))?
            .len();
        assert!(dot_len > 0, "Generated scenery.dot file is empty");
        Ok(())
    }

    #[test]
    fn test_repeated_serialization_preserves_embedded_materials() -> OpmResult<()> {
        let material_id = Uuid::new_v4();
        let const_refr = RefrIndexConst::new(1.5)?;
        let material = Material::new_for_test(material_id, 1, "N-BK7 Test", const_refr.into());

        let mut scenery = NodeGroup::default();
        let lens = Lens::new(
            "Test Lens",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            material,
        )?;
        scenery.add_node(lens)?;

        let doc = OpmDocument::new(scenery);
        let first_ron = doc.to_opm_file_string()?;
        assert!(first_ron.contains("embedded_materials:"));

        let second_ron = doc.to_opm_file_string()?;
        assert!(second_ron.contains("embedded_materials:"));

        let reloaded_doc = OpmDocument::from_string(&second_ron)?;
        assert_eq!(reloaded_doc.embedded_materials.len(), 1);

        Ok(())
    }

    #[test]
    fn test_corrupt_node_property_falls_back_to_default_and_warns() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "131024b9-f447-476d-ace2-b2c027ba0ef3",
        "props": {
            "expand view": Bool(false),
        },
        "graph": (
            nodes: [
                {
                    "node_type": "paraxial surface",
                    "name": "paraxial surface",
                    "uuid": "74e7f1d3-2315-4479-9649-21afb3a18e3a",
                    "props": {
                        "focal length": ength(0.01),
                    },
                    "gui_position": Some(-65.0, -40.17220926998195),
                },
            ],
            edges: [],
        ),
    },
    global: (
         ambient_material: {
            "schema_version": 1,
            "id": "6c30ef98-7380-4477-bc91-a5a1a407fec7",
            "version": 0,
            "name": "Vaccumm",
            "optical": (
                refractive_index: Const(
                    refractive_index: 1.0,
                ),
                absorption: r#None,
            ),
        },
    ),
)"#;

        testing_logger::setup();
        let doc = OpmDocument::from_string(ron_data)?;

        assert_eq!(doc.scenery().nodes().len(), 1);

        let node_ref = &doc.scenery().nodes()[0];
        let focal_length_prop = node_ref.node_attr().get_property("focal length")?;
        assert!(matches!(focal_length_prop, Proptype::Length(_)));

        check_logs(
            log::Level::Warn,
            vec!["Skipping property 'focal length' that failed to parse; keeping default value."],
        );

        Ok(())
    }

    #[test]
    fn test_nested_group_material_serialization_roundtrip() -> OpmResult<()> {
        let material_id = Uuid::new_v4();
        let const_refr = RefrIndexConst::new(1.5)?;
        let material =
            Material::new_for_test(material_id, 1, "Fused Silica Nested", const_refr.into());

        let mut inner_group = NodeGroup::new("Inner Group");
        let lens = Lens::new(
            "Nested Lens",
            millimeter!(100.0),
            millimeter!(-100.0),
            millimeter!(10.0),
            material,
        )?;
        inner_group.add_node(lens)?;

        let mut scenery = NodeGroup::new("Top Scenery");
        scenery.add_node(inner_group)?;

        let doc = OpmDocument::new(scenery);
        let ron_str = doc.to_opm_file_string()?;

        assert!(ron_str.contains("embedded_materials:"));
        assert!(ron_str.contains("Fused Silica Nested"));
        assert!(ron_str.contains("Id("));
        assert!(!ron_str.contains("Inline("));

        let reloaded_doc = OpmDocument::from_string(&ron_str)?;
        assert_eq!(reloaded_doc.embedded_materials.len(), 1);

        let all_reloaded_nodes = reloaded_doc.scenery().collect_all_nodes_recursive()?;
        let mut found_restored_material = false;

        for node_ref in all_reloaded_nodes {
            if let Ok(Proptype::Material(AssetRef::Inline(mat))) =
                node_ref.node_attr().get_property(MATERIAL)
            {
                assert_eq!(mat.id(), material_id);
                assert_eq!(mat.name(), "Fused Silica Nested");
                found_restored_material = true;
            }
        }

        assert!(
            found_restored_material,
            "Nested material was not properly re-hydrated"
        );

        Ok(())
    }

    #[test]
    fn test_load_source_port_with_empty_transform() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "91574253-61bc-4841-a61f-2f00de581ee9",
        "props": {
            "expand view": Bool(false),
        },
        "graph": (
            nodes: [
                {
                    "node_type": "source port",
                    "name": "source port",
                    "uuid": "b5502058-f129-4823-a726-4219f6abcc2e",
                    "isometry": Some(
                        transform: (),
                    ),
                    "gui_position": Some(290.0, 81.32779073001805),
                },
            ],
            edges: [],
        ),
    },
)"#;

        let doc = OpmDocument::from_string(ron_data)?;
        assert_eq!(
            doc.scenery().nodes().len(),
            1,
            "SourcePort with transform: () must be loaded successfully without being skipped"
        );

        let node_ref = &doc.scenery().nodes()[0];
        assert_eq!(
            node_ref.node_attr().positioning(),
            &NodePositioning::Automatic(None)
        );

        Ok(())
    }

    #[test]
    fn test_deserialize_node_with_asymmetric_ports() -> OpmResult<()> {
        let ron_data = r#"#![enable(unwrap_variant_newtypes)]
(
    opm_file_version: "0",
    scenery: {
        "node_type": "group",
        "name": "test",
        "uuid": "a0d4fdf0-75c4-470d-a69e-8e55619fcf8a",
        "props": {
            "expand view": Bool(false),
        },
        "graph": (
            nodes: [
                {
                    "node_type": "paraxial surface",
                    "name": "paraxial surface",
                    "ports": (
                        inputs: {
                            "input_1": (
                                aperture: (
                                    shape: BinaryCircle(
                                        radius: 0.025,
                                    ),
                                    a_type: Hole,
                                ),
                            ),
                        },
                        outputs: {
                            "output_1": (),
                        },
                    ),
                    "uuid": "5de2825b-9c2a-40bd-a87f-2f7a11d8be82",
                    "props": {
                        "focal length": Length(0.01),
                    },
                },
            ],
            edges: [],
        ),
    },
)"#;

        testing_logger::setup();
        let doc = OpmDocument::from_string(ron_data)?;

        assert_eq!(
            doc.scenery().nodes().len(),
            1,
            "Node with asymmetric ports must be successfully loaded and not skipped"
        );

        let node_ref = &doc.scenery().nodes()[0];
        assert_eq!(node_ref.node_attr().name(), "paraxial surface");
        check_logs(log::Level::Warn, vec![]);

        Ok(())
    }

    #[test]
    fn all_nodes_roundtrip_is_stable() -> OpmResult<()> {
        let original = fs::read_to_string("./files_for_testing/opm/all_nodes_roundtrip.opm")
            .map_err(|e| OpossumError::OpmDocument(format!("cannot read fixture file: {e}")))?;

        let doc = OpmDocument::from_string(&original)?;
        let roundtripped = doc.to_opm_file_string()?;

        assert_eq!(
            roundtripped, original,
            "loading and re-saving the fixture must reproduce it byte-for-byte; if this is an \
             intentional schema change, regenerate the fixture with `cargo run -p opossum_core \
             --example all_nodes_roundtrip_fixture` and re-check it in"
        );
        Ok(())
    }

    #[test]
    fn all_nodes_integration_test() -> OpmResult<()> {
        let mut scenery = NodeGroup::default();
        let i_0 = scenery.add_node(SourcePort::default())?;
        let i_1 = scenery.add_node(BeamSplitter::default())?;
        let i_2 = scenery.add_node(CylindricLens::default())?;
        let i_3 = scenery.add_node(FluenceDetector::default())?;
        let i_4 = scenery.add_node(Lens::default())?;
        let i_5 = scenery.add_node(Wedge::default())?;
        let i_6 = scenery.add_node(Dummy::default())?;
        let i_7 = scenery.add_node(EnergyMeter::default())?;
        let i_8 = scenery.add_node(IdealFilter::default())?;
        let i_9 = scenery.add_node(ParaxialSurface::new("paraxial", millimeter!(1000.0))?)?;
        let i_10 = scenery.add_node(RayPropagationVisualizer::default())?;
        let i_11 = scenery.add_node(Spectrometer::default())?;
        let i_12 = scenery.add_node(SpotDiagram::default())?;
        let i_13 = scenery.add_node(WaveFront::default())?;
        let i_14 = scenery.add_node(ParabolicMirror::default())?;
        let i_15 = scenery.add_node(
            ReflectiveGrating::default().with_rot_from_littrow(nanometer!(1000.0), degree!(0.0))?,
        )?;
        let i_16 = scenery.add_node(ThinMirror::default())?;

        scenery.connect_nodes(i_0, "output_1", i_1, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_1, "out1_trans1_refl2", i_2, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_2, "output_1", i_3, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_3, "output_1", i_4, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_4, "output_1", i_5, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_5, "output_1", i_6, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_6, "output_1", i_7, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_7, "output_1", i_8, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_8, "output_1", i_9, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_9, "output_1", i_10, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_10, "output_1", i_11, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_11, "output_1", i_12, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_12, "output_1", i_13, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_13, "output_1", i_14, "input_1", millimeter!(5.0))?;
        scenery.connect_nodes(i_14, "output_1", i_15, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(i_15, "output_1", i_16, "input_1", millimeter!(50.0))?;

        let ray_builder = round_collimated_ray_builder(millimeter!(10.0), joule!(1.0), 1)?;
        let mut config = RayTraceConfig::default();
        config.map_source(i_0, ray_builder.clone());

        testing_logger::setup();
        let analyzer = RayTracingAnalyzer::new(config);
        analyzer.analyze(&mut scenery)?;
        check_logs(log::Level::Warn, vec![]);
        scenery.reset_data();

        let mut config = GhostFocusConfig::default();
        config.map_source(i_0, ray_builder);
        let analyzer = GhostFocusAnalyzer::new(config);
        analyzer.analyze(&mut scenery)?;
        check_logs(log::Level::Warn, vec![]);

        Ok(())
    }

    #[test]
    fn full_analysis_with_save_and_load() -> OpmResult<()> {
        let mut scenery = NodeGroup::new("Lens Ray-trace test");
        let src = scenery.add_node(SourcePort::default())?;
        let lens1 = Wedge::new(
            "Wedge",
            millimeter!(10.0),
            degree!(0.0),
            &RefrIndexConst::new(1.5068)?,
        )?
        .with_tilt(degree!(15.0, 0.0, 0.0))?;
        let l1 = scenery.add_node(lens1)?;
        let lens2 = Lens::new(
            "Lens 2",
            millimeter!(205.55),
            millimeter!(-205.55),
            millimeter!(2.79),
            &RefrIndexConst::new(1.5068)?,
        )?
        .with_tilt(degree!(15.0, 0.0, 0.0))?;
        let l2 = scenery.add_node(lens2)?;
        let det = scenery.add_node(RayPropagationVisualizer::new("Ray plot", None)?)?;
        scenery.connect_nodes(src, "output_1", l1, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(l1, "output_1", l2, "input_1", millimeter!(50.0))?;
        scenery.connect_nodes(l2, "output_1", det, "input_1", millimeter!(50.0))?;

        let mut doc = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        config.map_source(
            src,
            collimated_line_ray_builder(millimeter!(20.0), joule!(1.0), 6)?,
        );
        doc.add_analyzer(AnalyzerType::RayTrace(config));

        let temp_model_file = NamedTempFile::new()
            .map_err(|e| OpossumError::OpmDocument(format!("Error generating temp file: {e}")))?;
        doc.save_to_file(temp_model_file.path())?;

        testing_logger::setup();
        let mut doc = OpmDocument::from_file(temp_model_file.path())?;
        let _ = doc.analyze()?;
        check_logs(log::Level::Warn, vec![]);
        Ok(())
    }
}
