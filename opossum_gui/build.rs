use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Copies / Renames a single external binary package (adds a target triple).
fn process_external_bin(
    pkg_name: &str,
    target_triple: &str,
    target_profile_dir: &Path,
    exe_suffix: &str,
) {
    println!("cargo:warning=Processing external binary: {pkg_name}");
    let exe_name = format!("{pkg_name}{exe_suffix}");
    let src_path = target_profile_dir.join(&exe_name);

    let dest_name = format!("{pkg_name}-{target_triple}{exe_suffix}");
    let dest_path = target_profile_dir.join(&dest_name);

    // Copy / Rename the file
    if src_path.exists() {
        fs::copy(&src_path, &dest_path).unwrap_or_else(|err| {
            panic!(
                "Failed to copy {pkg_name} binary from {} to {}: {err}",
                src_path.display(),
                dest_path.display(),
            )
        });
        println!(
            "cargo:warning=Copied {pkg_name} binary to {}",
            dest_path.display()
        );
    } else {
        panic!(
            "Could not find built binary for {pkg_name} at: {}.",
            src_path.display(),
        );
    }
}

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
    // Check if the 'bundle-backend' feature is enabled
    if env::var("CARGO_FEATURE_BUNDLE_BACKEND").is_ok() {
        println!("cargo:warning='bundle-backend' feature detected. Building external binaries...");

        // --- Get common build info ---
        let profile = env::var("PROFILE").expect("PROFILE env var not set by Cargo");
        let target_triple = env::var("TARGET").expect("TARGET env var not set by Cargo");
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let workspace_root = manifest_dir.parent().expect("Failed to get workspace root");

        let exe_suffix = if target_triple.contains("windows") {
            ".exe"
        } else {
            ""
        };

        let target_profile_dir = workspace_root.join("target").join(&profile);

        process_external_bin(
            "opossum_backend",
            &target_triple,
            &target_profile_dir,
            exe_suffix,
        );

        process_external_bin(
            "opossum_cli",
            &target_triple,
            &target_profile_dir,
            exe_suffix,
        );
    }
}
