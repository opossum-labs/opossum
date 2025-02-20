// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use app_lib::{commands, OPMGUIModel};
use opossum::OpmDocument;
use tauri::{
    generate_handler,
    menu::{MenuBuilder, SubmenuBuilder},
    Emitter, Manager,
};
use tauri_plugin_dialog::DialogExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(OPMGUIModel::new("generic model name"))))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(generate_handler![
            commands::add_node,
            commands::get_node_info,
            commands::set_inverted,
            commands::set_name,
            commands::set_lidt,
            commands::connect_nodes,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }
            let file_menu = SubmenuBuilder::new(app, "File")
                .text("open", "Open")
                .text("save", "Save")
                .build()?;

            let about_menu = SubmenuBuilder::new(app, "About")
                .text("about", "About")
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&file_menu, &about_menu])
                .build()?;

            app.set_menu(menu)?;

            app.on_menu_event(move |app_handle: &tauri::AppHandle, event| {
                match event.id().0.as_str() {
                    "open" => {
                        let opmgui_model =
                            Arc::clone(&app_handle.state::<Arc<Mutex<OPMGUIModel>>>());
                        if let Some(fp) = app_handle
                            .dialog()
                            .file()
                            .set_title("Open OPM file")
                            .add_filter("Opossum files", &["opm"])
                            .blocking_pick_file()
                        {
                            if let Ok(opm_doc) = OpmDocument::from_file(fp.as_path().unwrap()) {
                                let mut model = opmgui_model.lock().unwrap();
                                let scenery = opm_doc.scenery().clone();
                                model.set_model(opm_doc.scenery().clone());

                                println!("file loaded:{}", fp.to_string());
                                if let Ok(()) = app_handle.emit(
                                    "openopmfile",
                                    serde_json::to_string(&scenery).map_err(|e| e.to_string()),
                                ) {
                                    println!("file send");
                                } else {
                                    println!("file fucked");
                                }
                            }
                        } else {
                            println!("no file selected");
                        }
                        // move |fpath| {
                        //         if let Some(fp) = fpath {
                        //             if let Ok(opm_doc) =
                        //                 OpmDocument::from_file(fp.as_path().unwrap())
                        //             {
                        //                 let mut model = opmgui_model.lock().unwrap();
                        //                 let scenery = opm_doc.scenery().clone();
                        //                 model.set_model(opm_doc.scenery().clone());

                        //                 println!("file loaded:{}", fp.to_string());
                        //             }
                        //         }
                        //     });
                        //     if let Ok(e) = app_handle.emit("open_opm_file",serde_json::to_string(app_handle.state::<Arc<Mutex<OPMGUIModel>>>().lock().unwrap().model()).map_err(|e| e.to_string())){
                        //         println!("emission success")
                        //     }
                        //     else{
                        //         println!("emission failure");
                        //     }
                    }
                    "save" => {
                        if let Ok(opmgui_model) =
                            app_handle.state::<Arc<Mutex<OPMGUIModel>>>().lock()
                        {
                            let model = opmgui_model.model().clone();
                            app_handle
                                .dialog()
                                .file()
                                .set_title("Save OPM file")
                                .add_filter("Opossum files", &["opm"])
                                .save_file(|fpath| {
                                    if let Some(path) = fpath {
                                        let doc = OpmDocument::new(model);
                                        doc.save_to_file(&path.as_path().unwrap());
                                        println!("file saved:{}", path.to_string());
                                    } else {
                                        println!("no file selected");
                                    };
                                });
                        } else {
                            println!("could not lock state mutex");
                        }
                    }
                    "about" => {
                        println!("about event");
                    }
                    _ => {
                        println!("unexpected menu event");
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// fn save_file()
