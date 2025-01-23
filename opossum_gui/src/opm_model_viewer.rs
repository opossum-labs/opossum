use eframe::egui::{self, Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use log::info;

use crate::demo_node::DemoNode;

const LIGHT_RESULT_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
pub struct OPMModelViewer;

impl SnarlViewer<DemoNode> for OPMModelViewer {
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<DemoNode>) {
        match (&snarl[from.id.node], &snarl[to.id.node]) {
            (DemoNode::Source, DemoNode::Lens) => {}
            (DemoNode::Lens, DemoNode::Sink) => {}
            _ => {
                return;
            }
        }
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
        snarl.connect(from.id, to.id);
    }
    fn title(&mut self, node: &DemoNode) -> String {
        match node {
            DemoNode::Sink => "Sink".to_owned(),
            DemoNode::Source => "Source".to_owned(),
            DemoNode::Lens => "Lens".to_owned(),
            // DemoNode::String(_) => "String".to_owned(),
            // DemoNode::ShowImage(_) => "Show image".to_owned(),
            // DemoNode::ExprNode(_) => "Expr".to_owned(),
        }
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 1,
            DemoNode::Source => 0,
            DemoNode::Lens => 1,
        }
    }
    fn show_input(
        &mut self,
        pin: &egui_snarl::InPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<DemoNode>,
    ) -> egui_snarl::ui::PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink | DemoNode::Lens => {
                ui.label("input_1");
                assert_eq!(pin.id.input, 0, "Sink node has only one input");

                match &*pin.remotes {
                    [] => PinInfo::circle().with_fill(UNTYPED_COLOR),
                    [remote] => match snarl[remote.node] {
                        DemoNode::Sink => unreachable!("Sink node has no outputs"),
                        DemoNode::Source => {
                            assert_eq!(remote.output, 0, "Number node has only one output");
                            PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
                        }
                        DemoNode::Lens => {
                            assert_eq!(remote.output, 0, "Number node has only one output");
                            PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
                        }
                    },
                    _ => unreachable!("Sink input has only one wire"),
                }
            }
            DemoNode::Source => {
                unreachable!("Source node has no inputs");
            }
        }
    }

    fn outputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 0,
            DemoNode::Source => 1,
            DemoNode::Lens => 1,
        }
    }

    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<DemoNode>,
    ) -> egui_snarl::ui::PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                unreachable!("Sink node has no outputs")
            }
            DemoNode::Source | DemoNode::Lens => {
                ui.label("ouput_1");
                assert_eq!(pin.id.output, 0, "Source node has only one output");
                PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
            }
        }
    }
    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<DemoNode>) -> bool {
        true
    }
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Add node");
        if ui.button("Source").clicked() {
            snarl.insert_node(pos, DemoNode::Source);
            ui.close_menu();
        }
        if ui.button("Lens").clicked() {
            snarl.insert_node(pos, DemoNode::Lens);
            ui.close_menu();
        }
        if ui.button("Sink").clicked() {
            snarl.insert_node(pos, DemoNode::Sink);
            ui.close_menu();
        }
    }
    fn has_node_menu(&mut self, _node: &DemoNode) -> bool {
        true
    }
    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
    }
    fn has_on_hover_popup(&mut self, _: &DemoNode) -> bool {
        true
    }

    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<DemoNode>,
    ) {
        match snarl[node] {
            DemoNode::Sink => {
                ui.label("Demo sink node");
            }
            DemoNode::Source => {
                ui.label("Optical source emitting radiation");
            }
            DemoNode::Lens => {
                ui.label("Lens node");
            }
        }
    }
    fn header_frame(
        &mut self,
        frame: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<DemoNode>,
    ) -> egui::Frame {
        match snarl[node] {
            DemoNode::Sink => frame.fill(egui::Color32::from_rgb(150, 150, 150)),
            DemoNode::Source => frame.fill(egui::Color32::from_rgb(200, 150, 150)),
            DemoNode::Lens => frame.fill(egui::Color32::from_rgb(150, 200, 150)),
        }
    }
    fn has_wire_widget(
        &mut self,
        _from: &OutPinId,
        _to: &InPinId,
        _snarl: &Snarl<DemoNode>,
    ) -> bool {
        info!("show_wire_widget");
        true
    }
    fn show_wire_widget(
        &mut self,
        _from: &OutPin,
        _to: &InPin,
        ui: &mut Ui,
        _scale: f32,
        _snarl: &mut Snarl<DemoNode>,
    ) {
        info!("show_wire_widget");
        ui.label("Wdiget");
    }
}
