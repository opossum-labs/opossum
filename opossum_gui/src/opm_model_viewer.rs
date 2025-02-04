use std::vec;

use crate::editor_node::EditorNode;
use eframe::egui::{self, Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, NodeId, OutPin, Snarl,
};
use log::{error, info};
use opossum::{
    analyzers::AnalyzerType,
    millimeter,
    nodes::{create_node_ref, NodeGroup},
    optic_ports::PortType,
};

const LIGHT_RESULT_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
#[derive(Default)]
pub struct OPMModelViewer {
    model: NodeGroup,
    analyzers: Vec<AnalyzerType>,
}
impl OPMModelViewer {
    pub fn model(&self) -> &NodeGroup {
        &self.model
    }
    pub fn analyzers(&self) -> &[AnalyzerType] {
        &self.analyzers
    }
    pub fn gen_properties_gui(&mut self, ui: &mut Ui, ctx: &egui::Context, node_id: NodeId, snarl: &Snarl<EditorNode>) {
        match snarl[node_id] {
            EditorNode::OpticRef(ref node) => {
                let mut node_ref = node.optical_ref.borrow_mut();
                let node_attr = node_ref.node_attr_mut();
                node_attr.generate_gui(ui,ctx);
            }
            EditorNode::Analyzer(ref node) => {
                ui.label(format!("Analyzer: {:?}", node));
            }
        }
    }
}

impl SnarlViewer<EditorNode> for OPMModelViewer {
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<EditorNode>) {
        let EditorNode::OpticRef(from_node) = snarl[from.id.node].clone() else {
            error!("Trying to connenct from non-optical node");
            return;
        };
        let EditorNode::OpticRef(to_node) = snarl[to.id.node].clone() else {
            error!("Trying to connenct to non-optical node");
            return;
        };
        let src_uuid = from_node.uuid();
        let target_uuid = to_node.uuid();
        let src_port = from_node
            .optical_ref
            .borrow()
            .ports()
            .names(&PortType::Output)[from.id.output]
            .to_owned();
        let target_port =
            to_node.optical_ref.borrow().ports().names(&PortType::Input)[to.id.input].to_owned();
        if let Err(e) = self.model.connect_nodes_by_uuid(
            src_uuid,
            &src_port,
            target_uuid,
            &target_port,
            millimeter!(10.0),
        ) {
            error!("Error connecting nodes: {:?}", e);
            return;
        }
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
        snarl.connect(from.id, to.id);
    }
    fn title(&mut self, node: &EditorNode) -> String {
        match node {
            EditorNode::OpticRef(node) => node.optical_ref.borrow().name(),
            EditorNode::Analyzer(node) => format!("{}", node),
        }
    }
    fn inputs(&mut self, node: &EditorNode) -> usize {
        match node {
            EditorNode::OpticRef(node) => node
                .optical_ref
                .borrow()
                .ports()
                .names(&PortType::Input)
                .len(),
            EditorNode::Analyzer(_) => 0,
        }
    }
    fn show_input(
        &mut self,
        pin: &egui_snarl::InPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<EditorNode>,
    ) -> egui_snarl::ui::PinInfo {
        if let EditorNode::OpticRef(node) = &snarl[pin.id.node] {
            let node = node.optical_ref.borrow();
            let port_names = node.ports().names(&PortType::Input);
            let i = pin.id.input;
            ui.label(port_names[i].to_owned());
            PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
        } else {
            PinInfo::circle().with_fill(UNTYPED_COLOR)
        }
    }
    fn outputs(&mut self, node: &EditorNode) -> usize {
        match node {
            EditorNode::OpticRef(node) => node
                .optical_ref
                .borrow()
                .ports()
                .names(&PortType::Output)
                .len(),
            EditorNode::Analyzer(_) => 0,
        }
    }
    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<EditorNode>,
    ) -> egui_snarl::ui::PinInfo {
        if let EditorNode::OpticRef(node) = &snarl[pin.id.node] {
            let node = node.optical_ref.borrow();
            let port_names = node.ports().names(&PortType::Output);
            let i = pin.id.output;
            ui.label(port_names[i].to_owned());
            PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
        } else {
            PinInfo::circle().with_fill(UNTYPED_COLOR)
        }
    }
    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<EditorNode>) -> bool {
        true
    }
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        ui.strong("Add node");
        if ui.button("gen test nodes").clicked() {
            for i in 0..250 {
                let node = create_node_ref("dummy").unwrap();
                self.model.add_node_ref(&node).unwrap();
                snarl.insert_node(
                    pos + egui::vec2((i % 15) as f32 * 160.0, (i / 15) as f32 * 60.0),
                    EditorNode::OpticRef(node),
                );
            }
            ui.close_menu();
        }
        let available_nodes = vec![
            "dummy",
            "beam splitter",
            "energy meter",
            "fluence detector",
            "group",
            "ideal filter",
            "lens",
            "cylindric lens",
            "mirror",
            "parabolic mirror",
            "paraxial surface",
            "ray propagation",
            "reference",
            "reflective grating",
            "source",
            "spectrometer",
            "spot diagram",
            "wavefront monitor",
            "wedge",
        ];
        for node in available_nodes {
            if ui.button(node).clicked() {
                let node = create_node_ref(node).unwrap();
                self.model.add_node_ref(&node).unwrap();
                snarl.insert_node(pos, EditorNode::OpticRef(node));
                ui.close_menu();
            }
        }
        ui.strong("Add analyzer");
        let available_analyzers = vec!["Energy", "RayTrace", "GhostFocus"];
        for analyzer in available_analyzers {
            if ui.button(analyzer).clicked() {
                if let Some(analyzer) = AnalyzerType::from_name(analyzer) {
                    self.analyzers.push(analyzer.clone());
                    snarl.insert_node(pos, EditorNode::Analyzer(analyzer));
                    ui.close_menu();
                } else {
                    error!("Unknown analyzer: {}", analyzer);
                }
            }
        }
    }
    fn has_node_menu(&mut self, _node: &EditorNode) -> bool {
        true
    }
    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        ui.strong("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
        if let Some(EditorNode::OpticRef(node)) = snarl.get_node(node) {
            if node.optical_ref.borrow().name() == "group" {
                if ui.button("Open group").clicked() {
                    info!("Open group {:?}", node.uuid());
                    ui.close_menu();
                }
            }
        }
    }
    fn has_on_hover_popup(&mut self, _: &EditorNode) -> bool {
        true
    }
    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<EditorNode>,
    ) {
        match snarl[node] {
            EditorNode::OpticRef(ref node) => {
                ui.label(format!("{:?}", node.optical_ref.borrow().name()));
            }
            EditorNode::Analyzer(ref node) => {
                ui.label(format!("{:?}", node));
            }
        }
    }
    fn header_frame(
        &mut self,
        frame: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<EditorNode>,
    ) -> egui::Frame {
        // snarl[node].optical_ref.borrow().node_color();
        let color = match snarl[node] {
            EditorNode::OpticRef(_) => Color32::from_rgb(150, 150, 150),
            EditorNode::Analyzer(_) => Color32::from_rgb(200, 150, 150),
        };
        frame.fill(color)
    }
}
