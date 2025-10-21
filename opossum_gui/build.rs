use std::{env, fs, path::{Path, PathBuf}, process::Command};

/// Builds and copies a single external binary package.
fn process_external_bin(
    pkg_name: &str,
    profile: &str,
    target_triple: &str,
    workspace_root: &Path,
    target_profile_dir: &Path,
    exe_suffix: &str,
) {
    println!("cargo:warning=Processing external binary: {}", pkg_name);

    // 1. Tell Cargo to rebuild if the package's src changes
    // Assumes the package is one level up, e.g., ../opossum_cli/src
    println!("cargo:rerun-if-changed=../{}/src", pkg_name);

    // // 2. Build the external binary package
    // let build_status = Command::new("cargo")
    //     .arg("build")
    //     .arg("--package")
    //     .arg(pkg_name)
    //     .args(if profile == "release" { vec!["--release"] } else { vec![] })
    //     .status()
    //     .unwrap_or_else(|e| panic!("Failed to execute build command for {}: {}", pkg_name, e));

    // if !build_status.success() {
    //     panic!("Build failed for package: {}", pkg_name);
    // }

    // 3. Determine file names and paths
    let exe_name = format!("{}{}", pkg_name, exe_suffix);
    let src_path = target_profile_dir.join(&exe_name);

    let dest_name = format!("{}-{}{}", pkg_name, target_triple, exe_suffix);
    let dest_path = target_profile_dir.join(&dest_name);

    // 4. Copy the file
    if src_path.exists() {
        fs::copy(&src_path, &dest_path).unwrap_or_else(|err| {
            panic!(
                "Failed to copy {} binary from {:?} to {:?}: {}",
                pkg_name, src_path, dest_path, err
            )
        });
        println!(
            "cargo:warning=Copied {} binary to {}",
            pkg_name,
            dest_path.display()
        );
    } else {
        panic!(
            "Could not find built binary for {} at: {}. Make sure the package name '{}' is correct and builds successfully.",
            pkg_name,
            src_path.display(),
            pkg_name
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

        let exe_suffix = if target_triple.contains("windows") { ".exe" } else { "" };

        let target_profile_dir = workspace_root
            .join("target")
            .join(&profile);

        // --- Process each external binary ---
        process_external_bin(
            "opossum_backend",
            &profile,
            &target_triple,
            &workspace_root,
            &target_profile_dir,
            exe_suffix,
        );
        
        process_external_bin(
            "opossum_cli",
            &profile,
            &target_triple,
            &workspace_root,
            &target_profile_dir,
            exe_suffix,
        );

    } else {
        // Feature is NOT set
        println!("cargo:warning=Skipping external binary builds. (Enable 'bundle-backend' feature to build).");
    }
}
