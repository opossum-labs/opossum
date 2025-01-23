use eframe::egui::{self, Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use log::info;

use crate::demo_node::DemoNode;

// const STRING_COLOR: Color32 = Color32::from_rgb(0x00, 0xb0, 0x00);
const NUMBER_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
// const IMAGE_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0xb0);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
pub struct DemoViewer;

impl SnarlViewer<DemoNode> for DemoViewer {
    fn title(&mut self, node: &DemoNode) -> String {
        match node {
            DemoNode::Sink => "Sink".to_owned(),
            DemoNode::Source => "Source".to_owned(),
            // DemoNode::String(_) => "String".to_owned(),
            // DemoNode::ShowImage(_) => "Show image".to_owned(),
            // DemoNode::ExprNode(_) => "Expr".to_owned(),
        }
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 1,
            DemoNode::Source => 0, // DemoNode::Number(_) | DemoNode::String(_) => 0,
                                   // DemoNode::ExprNode(expr_node) => 1 + expr_node.bindings.len(),
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
            DemoNode::Sink => {
                assert_eq!(pin.id.input, 0, "Sink node has only one input");

                match &*pin.remotes {
                    [] => {
                        ui.label("None");
                        PinInfo::circle().with_fill(UNTYPED_COLOR)
                    }
                    [remote] => match snarl[remote.node] {
                        DemoNode::Sink => unreachable!("Sink node has no outputs"),
                        DemoNode::Source => {
                            assert_eq!(remote.output, 0, "Number node has only one output");
                            //ui.label(format_float(value));
                            PinInfo::circle().with_fill(NUMBER_COLOR)
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
            DemoNode::Source => 1, // | DemoNode::String(_)
                                   // | DemoNode::ShowImage(_)
                                   // | DemoNode::ExprNode(_) => 1,
        }
    }

    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        _ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<DemoNode>,
    ) -> egui_snarl::ui::PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                unreachable!("Sink node has no outputs")
            }
            DemoNode::Source => {
                assert_eq!(pin.id.output, 0, "Source node has only one output");
                // ui.add(egui::DragValue::new(value));
                PinInfo::circle().with_fill(NUMBER_COLOR)
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
        // if ui.button("Expr").clicked() {
        //     snarl.insert_node(pos, DemoNode::ExprNode(ExprNode::new()));
        //     ui.close_menu();
        // }
        // if ui.button("String").clicked() {
        //     snarl.insert_node(pos, DemoNode::String(String::new()));
        //     ui.close_menu();
        // }
        // if ui.button("Show image").clicked() {
        //     snarl.insert_node(pos, DemoNode::ShowImage(String::new()));
        //     ui.close_menu();
        // }
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
            } // DemoNode::String(_) => {
              //     ui.label("Outputs string value");
              // }
              // DemoNode::ShowImage(_) => {
              //     ui.label("Displays image from URL in input");
              // }
              // DemoNode::ExprNode(_) => {
              //     ui.label("Evaluates algebraic expression with input for each unique variable name");
              // }
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
            // DemoNode::String(_) => frame.fill(egui::Color32::from_rgb(40, 70, 40)),
            // DemoNode::ShowImage(_) => frame.fill(egui::Color32::from_rgb(40, 40, 70)),
            // DemoNode::ExprNode(_) => frame.fill(egui::Color32::from_rgb(70, 66, 40)),
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
