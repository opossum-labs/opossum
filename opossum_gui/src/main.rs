#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
use eframe::egui;
use env_logger::Env;
use log::info;

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
    name: String,
    age: u32,
}
impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
        }
    }
}
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        info!("About clicked");
                    }
                });
            })
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            // ui.heading("Opossum");
            ui.hyperlink("https://www.gsi.de");
            ui.separator();
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));

            // ui.image(egui::include_image!(
            //     "../../../crates/egui/assets/ferris.png"
            // ));
        });
    }
}
