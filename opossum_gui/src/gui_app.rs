use std::path::PathBuf;

use eframe::egui::{self, Id};
use egui_file_dialog::FileDialog;
use egui_modal::{Icon, Modal, ModalStyle};
use egui_snarl::{ui::SnarlStyle, Snarl};

use crate::{demo_node::DemoNode, demo_viewer::DemoViewer};

pub struct GuiApp {
    snarl: Snarl<DemoNode>,
    style: SnarlStyle,
    snarl_ui_id: Option<Id>,
    file_dialog: FileDialog,
    picked_file: Option<PathBuf>,
}
impl Default for GuiApp {
    fn default() -> Self {
        Self {
            snarl: Snarl::default(),
            style: SnarlStyle::default(),
            snarl_ui_id: None,
            file_dialog: FileDialog::default(),
            picked_file: None,
        }
    }
}
impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let modal = Modal::new(ctx, "modal")
            .with_style(&ModalStyle::default())
            .with_close_on_outside_click(false);

        // the show function defines what is shown in the modal, but the modal
        // won't actually show until you do modal.open()
        modal.show(|ui| {
            modal.title(ui, "OPOSSUM");
            modal.icon(ui, Icon::Info);
            // the "frame" of the modal refers to the container of the icon and body.
            // this helper just applies a margin specified by the ModalStyle
            modal.frame(ui, |ui| {
                modal.body(ui, "This is the OPOSSUM Software");
            });

            modal.buttons(ui, |ui| {
                if modal.button(ui, "close").clicked() {
                    println!("hello world!");
                }
            });
        });
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        self.file_dialog.pick_file();
                    }
                    if ui.button("Quit").clicked() {
                        let ctx = ctx.clone();
                        std::thread::spawn(move || {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        });
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        modal.open();
                    }
                });
            })
        });
        egui::SidePanel::left("Properties").show(ctx, |ui| {
            ui.heading("Properties");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Picked file: {:?}", self.picked_file));

            self.snarl_ui_id = Some(ui.id());
            self.snarl.show(&mut DemoViewer, &self.style, "snarl", ui);
        });
        // Update the dialog
        self.file_dialog.update(ctx);
        // Check if the user picked a file.
        if let Some(path) = self.file_dialog.take_picked() {
            self.picked_file = Some(path.to_path_buf());
        }
    }
}