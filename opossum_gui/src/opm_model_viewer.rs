use std::vec;

use eframe::egui::{self, Color32, Ui};
use egui_snarl::{
    ui::{PinInfo, SnarlViewer},
    InPin, NodeId, OutPin, Snarl,
};
use log::info;
use opossum::{
    nodes::{create_node_ref, NodeGroup},
    optic_ports::PortType,
    optic_ref::OpticRef,
};

const LIGHT_RESULT_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
// const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);
#[derive(Default)]
pub struct OPMModelViewer {
    model: NodeGroup,
}

impl SnarlViewer<OpticRef> for OPMModelViewer {
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<OpticRef>) {
        // match (&snarl[from.id.node], &snarl[to.id.node]) {
        //     (OpticRef::Source, OpticRef::Lens) => {}
        //     (OpticRef::Lens, OpticRef::Sink) => {}
        //     _ => {
        //         return;
        //     }
        // }
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
        snarl.connect(from.id, to.id);
    }
    fn title(&mut self, node: &OpticRef) -> String {
        node.optical_ref.borrow().name()
    }
    fn inputs(&mut self, node: &OpticRef) -> usize {
        node.optical_ref
            .borrow()
            .ports()
            .names(&PortType::Input)
            .len()
    }
    fn show_input(
        &mut self,
        pin: &egui_snarl::InPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<OpticRef>,
    ) -> egui_snarl::ui::PinInfo {
        let node = snarl[pin.id.node].optical_ref.borrow();
        let port_names = node.ports().names(&PortType::Input);
        let i = pin.id.input;
        ui.label(port_names[i].to_owned());
        PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
    }
    fn outputs(&mut self, node: &OpticRef) -> usize {
        node.optical_ref
            .borrow()
            .ports()
            .names(&PortType::Output)
            .len()
    }
    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        ui: &mut eframe::egui::Ui,
        _scale: f32,
        snarl: &mut egui_snarl::Snarl<OpticRef>,
    ) -> egui_snarl::ui::PinInfo {
        let node = snarl[pin.id.node].optical_ref.borrow();
        let port_names = node.ports().names(&PortType::Output);
        let i = pin.id.output;
        ui.label(port_names[i].to_owned());
        PinInfo::circle().with_fill(LIGHT_RESULT_COLOR)
    }
    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<OpticRef>) -> bool {
        true
    }
    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<OpticRef>,
    ) {
        ui.strong("Add node");
        if ui.button("gen test nodes").clicked() {
            for i in 0..250 {
                let node = create_node_ref("dummy").unwrap();
                self.model.add_node_ref(&node).unwrap();
                snarl.insert_node(
                    pos + egui::vec2((i % 15) as f32 * 160.0, (i / 15) as f32 * 60.0),
                    node,
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
                snarl.insert_node(pos, node);
                ui.close_menu();
            }
        }
    }
    fn has_node_menu(&mut self, _node: &OpticRef) -> bool {
        true
    }
    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<OpticRef>,
    ) {
        ui.strong("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }
        if let Some(node) = snarl.get_node(node) {
            if node.optical_ref.borrow().name() == "group" {
                if ui.button("Open group").clicked() {
                    info!("Open group {:?}", node.uuid());
                    ui.close_menu();
                }
            }
        }
    }
    fn has_on_hover_popup(&mut self, _: &OpticRef) -> bool {
        true
    }
    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        _scale: f32,
        snarl: &mut Snarl<OpticRef>,
    ) {
        ui.label(format!("{:?}", snarl[node].optical_ref.borrow().name()));
    }
    fn header_frame(
        &mut self,
        frame: egui::Frame,
        _node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        _snarl: &Snarl<OpticRef>,
    ) -> egui::Frame {
        // snarl[node].optical_ref.borrow().node_color();
        frame.fill(egui::Color32::from_rgb(150, 150, 150))
    }
}
