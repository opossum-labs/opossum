use eframe::egui::{self, Color32, Ui};
use egui_snarl::{ui::{PinInfo, SnarlViewer}, Snarl};

use crate::demo_node::DemoNode;

// const STRING_COLOR: Color32 = Color32::from_rgb(0x00, 0xb0, 0x00);
// const NUMBER_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
// const IMAGE_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0xb0);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
pub struct DemoViewer;

impl SnarlViewer<DemoNode> for DemoViewer {
    fn title(&mut self, node: &DemoNode) -> String {
        match node {
            DemoNode::Sink => "Sink".to_owned(),
            // DemoNode::Number(_) => "Number".to_owned(),
            // DemoNode::String(_) => "String".to_owned(),
            // DemoNode::ShowImage(_) => "Show image".to_owned(),
            // DemoNode::ExprNode(_) => "Expr".to_owned(),
        }
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 1,
            // DemoNode::Number(_) | DemoNode::String(_) => 0,
            // DemoNode::ExprNode(expr_node) => 1 + expr_node.bindings.len(),
        }
    }

    fn show_input(&mut self, pin: &egui_snarl::InPin, ui: &mut eframe::egui::Ui, scale: f32, snarl: &mut egui_snarl::Snarl<DemoNode>)
        -> egui_snarl::ui::PinInfo {
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
                        },
                        _ => unreachable!("Sink input has only one wire"),
                    }
                }
            }
    }

    fn outputs(&mut self, node: &DemoNode) -> usize {
        match node {
            DemoNode::Sink => 0,
            // DemoNode::Number(_)
            // | DemoNode::String(_)
            // | DemoNode::ShowImage(_)
            // | DemoNode::ExprNode(_) => 1,
        }
    }

    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        ui: &mut eframe::egui::Ui,
        scale: f32,
        snarl: &mut egui_snarl::Snarl<DemoNode>,
    ) -> egui_snarl::ui::PinInfo {
        match snarl[pin.id.node] {
            DemoNode::Sink => {
                unreachable!("Sink node has no outputs")
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
        // if ui.button("Number").clicked() {
        //     snarl.insert_node(pos, DemoNode::Number(0.0));
        //     ui.close_menu();
        // }
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
}