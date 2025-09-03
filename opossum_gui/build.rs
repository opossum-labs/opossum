use std::env;
use std::fs;
use std::path::PathBuf;

// This build script is necessary in order to embed an application icon into the windows executable.
// Unfortunately this does not work with the standard dioxus bundler...

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/favicon.ico");

        // This is good practice for modern Windows applications
        res.set_manifest(
            r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
        <requestedPrivileges>
            <requestedExecutionLevel level="asInvoker" uiAccess="false" />
        </requestedPrivileges>
    </security>
</trustInfo>
</assembly>
"#,
        );
        if let Err(e) = res.compile() {
            eprintln!("Failed to compile Windows resources: {e}");
            std::process::exit(1);
        }
    }
    // Part 2: Generate the final .wxs file from the template
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().expect("Failed to get workspace root");

    // Calculate the absolute path to the release directory
    let release_dir_abs = workspace_root.join("target").join("release");

    // Define the paths for the template and the final output file
    let template_path = manifest_dir.join("packaging").join("extra_binaries.wxs.in");
    let final_wxs_path = manifest_dir.join("packaging").join("extra_binaries.wxs");

    // Read the template content
    let template_content =
        fs::read_to_string(&template_path).expect("Failed to read extra_binaries.wxs.in");

    // Replace the placeholder with the absolute path
    // .display().to_string() ensures correct path separators for Windows
    let final_content =
        template_content.replace("@@RELEASE_DIR@@", &release_dir_abs.display().to_string());

    // Write the final .wxs file that the bundler will use
    fs::write(&final_wxs_path, final_content).expect("Failed to write final extra_binaries.wxs");
}
