use crate::OPOSSUM_UI_LOGS;
use dioxus::prelude::*;
use rfd::{AsyncFileDialog, FileDialog};
use std::fs;

pub fn use_new_project() -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let _path = FileDialog::new()
            .set_directory("/")
            .set_title("Select new OPOSSUM project directoy")
            .pick_folder();
    })
}

pub fn use_open_file() -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let _path = FileDialog::new()
            .set_directory("/")
            .set_title("Select OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .pick_file();
    })
}

pub fn use_save_project() {
    // use_resource({
    //      move ||
    // spawn(async move {
    //         let _path = FileDialog::new()
    //             .set_directory("/")
    //             .set_title("Save OPOSSUM setup file")
    //             .add_filter("Opossum setup file", &["opm"])
    //             .save_file();});
    //    }
    // });
    let future = async {
        let file = AsyncFileDialog::new()
            .add_filter("text", &["txt", "rs"])
            .add_filter("rust", &["rs", "toml"])
            .set_directory("/")
            .pick_file()
            .await;

        let data = file.unwrap().read().await;
    };
}

pub fn use_open_project() -> Callback<Event<MouseData>> {
    use_callback(move |_| {
        let path = FileDialog::new()
            .set_directory("/")
            .set_title("Open existing OPOSSUM project directory")
            .pick_folder();
        if let Some(p) = path {
            if let Ok(paths) = fs::read_dir(p) {
                let mut opm_found = false;
                for pp in paths.flatten() {
                    if pp.path().ends_with(".opm") {
                        opm_found = true;
                        break;
                    }
                }
                if opm_found {
                    todo!();
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("No opm file found, cannot open project!");
                }
            }
        }
    })
}
