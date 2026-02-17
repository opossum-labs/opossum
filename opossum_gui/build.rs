// This build script is necessary in order to embed an application icon into the windows executable.
// Unfortunately this does not work with the standard dioxus bundler...

fn main() {
    // set program icon on windows
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/favicon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {e}");
            std::process::exit(1);
        }
    }
}
