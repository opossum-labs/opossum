use std::sync::Arc;

use crate::opm_model_viewer::OPMModelViewer;
use eframe::egui::{self, Id};
use egui_file_dialog::FileDialog;
use egui_modal::{Icon, Modal, ModalStyle};
use egui_snarl::{ui::SnarlStyle, Snarl};
use log::info;
use opossum::{
    analyzers::{
        energy::EnergyAnalyzer, ghostfocus::GhostFocusAnalyzer, raytrace::RayTracingAnalyzer,
        Analyzer, AnalyzerType,
    },
    optic_node::OpticNode,
    optic_ref::OpticRef,
    OpmDocument,
};

pub struct GuiApp {
    opm_document: OpmDocument,
    snarl: Snarl<OpticRef>,
    style: SnarlStyle,
    snarl_ui_id: Option<Id>,
    file_dialog: FileDialog,
    snarl_viewer: OPMModelViewer,
}
impl Default for GuiApp {
    fn default() -> Self {
        Self {
            opm_document: OpmDocument::default(),
            snarl: Snarl::default(),
            style: SnarlStyle::default(),
            snarl_ui_id: None,
            file_dialog: FileDialog::default()
                .add_file_filter(
                    "OPOSSUM models",
                    Arc::new(|p| p.extension().unwrap_or_default() == "opm"),
                )
                .default_file_filter("OPOSSUM models"),
            snarl_viewer: OPMModelViewer::default(),
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
                    if ui.button("Save").clicked() {
                        self.file_dialog.save_file();
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
        egui::TopBottomPanel::bottom("Log")
            .resizable(true)
            .show(ctx, |ui| {
                egui_logger::logger_ui()
                    .show_target(false)
                    .enable_ctx_menu(false)
                    .enable_regex(false)
                    .show(ui);
            });

        if let Some(snarl_ui_id) = self.snarl_ui_id {
            egui::SidePanel::left("Properties").show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Properties");

                    let selected =
                        Snarl::<OpticRef>::get_selected_nodes_at("snarl", snarl_ui_id, ui.ctx());
                    let mut selected = selected
                        .into_iter()
                        .map(|id| (id, &self.snarl[id]))
                        .collect::<Vec<_>>();

                    selected.sort_by_key(|(id, _)| *id);

                    for (id, _node) in selected {
                        ui.horizontal(|ui| {
                            ui.label(format!("{id:?}"));
                            // ui.label(node.name());
                            ui.add_space(ui.spacing().item_spacing.x);
                        });
                    }
                });
            });
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            self.snarl_ui_id = Some(ui.id());
            self.snarl
                .show(&mut self.snarl_viewer, &self.style, "snarl", ui);
        });
        // Update the dialog
        self.file_dialog.update(ctx);
        // Check if the user picked a file.
        if let Some(path) = self.file_dialog.take_picked() {
            match self.file_dialog.mode() {
                egui_file_dialog::DialogMode::SelectFile => {
                    self.opm_document = OpmDocument::from_file(&path).unwrap();
                    let analyzers = self.opm_document.analyzers();
                    let scenery = self.opm_document.scenery_mut();
                    if analyzers.is_empty() {
                        info!("No analyzer defined in document. Stopping here.");
                    } else {
                        for ana in analyzers.iter().enumerate() {
                            let analyzer: &dyn Analyzer = match ana.1 {
                                AnalyzerType::Energy => &EnergyAnalyzer::default(),
                                AnalyzerType::RayTrace(config) => {
                                    &RayTracingAnalyzer::new(config.clone())
                                }
                                AnalyzerType::GhostFocus(config) => {
                                    &GhostFocusAnalyzer::new(config.clone())
                                }
                                _ => &EnergyAnalyzer::default(),
                            };
                            info!("Analysis #{}", ana.0);
                            scenery.clear_edges();
                            scenery.reset_data();
                            analyzer.analyze(scenery).unwrap()
                        }
                    }
                }
                egui_file_dialog::DialogMode::SelectDirectory => todo!(),
                egui_file_dialog::DialogMode::SelectMultiple => todo!(),
                egui_file_dialog::DialogMode::SaveFile => {
                    self.opm_document=OpmDocument::new(self.snarl_viewer.model().clone());
                    info!("Saving file to {:?}", path);
                    self.opm_document.save_to_file(&path).unwrap();
                }
            }
        }
    }
}
