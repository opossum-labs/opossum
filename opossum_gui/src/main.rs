#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use eframe::egui::{self, Id};
use egui_modal::{Icon, Modal, ModalStyle};
use egui_snarl::{ui::SnarlStyle, Snarl};
use env_logger::Env;
use log::info;
use opossum_gui::{demo_node::DemoNode, demo_viewer::DemoViewer};

fn main() -> eframe::Result {
    // by default, log everything from level `info` and up.
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    // let options = eframe::NativeOptions {
    //     viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
    //     ..Default::default()
    // };
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Opossum",
        options,
        Box::new(|_cc| {
            // This gives us image support:
            // egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<MyApp>::default())
        }),
    )
}
struct MyApp {
    snarl: Snarl<DemoNode>,
    style: SnarlStyle,
    snarl_ui_id: Option<Id>,
}
impl Default for MyApp {
    fn default() -> Self {
        Self {
            snarl: Snarl::default(),
            style: SnarlStyle::default(),
            snarl_ui_id: None
        }
    }
}
impl eframe::App for MyApp {
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
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        modal.open();
                        info!("About clicked");
                    }
                });
            })
        });
        egui::SidePanel::left("Properties").show(ctx, |ui| {
            ui.heading("Properties");
        });
        egui::SidePanel::right("Elements").show(ctx, |ui| {
            ui.heading("Elements");
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.snarl_ui_id = Some(ui.id());
            self.snarl.show(&mut DemoViewer, &self.style, "snarl", ui);
        });
    }
}
