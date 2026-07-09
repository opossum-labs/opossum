use clap::{Parser, Subcommand};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use xshell::{Shell, cmd};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Automation tasks for OPOSSUM", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the complete Installer-Bundle (CLI, Backend, GUI)
    Bundle,
    /// Perform tests, lints and formatting checks (CI Pipeline)
    Ci,
    /// Create a coverage report (requires grcov)
    Coverage,
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    match cli.command {
        // Unser eigener Task
        Commands::Bundle => task_bundle()?,
        Commands::Ci => xtaskops::tasks::ci()?,
        Commands::Coverage => xtaskops::tasks::coverage(true)?,
    }
    Ok(())
}

struct StagingGuard {
    path: PathBuf,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            println!("🧹 Cleaning up staging directory...");
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

struct ExamplesGuard {
    path: PathBuf,
}

impl Drop for ExamplesGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            println!("🧹 Cleaning up temporary examples directory...");
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn task_bundle() -> Result<(), anyhow::Error> {
    let sh = Shell::new()?;
    println!("🦀 Start build process for OPOSSUM...");
    // 1. Derive host triple
    let target_triple = get_host_target_triple()?;
    println!("🎯 Target Triple: {}", target_triple);
    // 2. build binaries (Release)
    {
        println!("\n📦 Building binaries...");
        cmd!(sh, "cargo build --release -p opossum_cli").run()?;
        cmd!(sh, "cargo build --release -p opossum_backend").run()?;
    }
    // 3. Create staging and examples area and rename/copy binaries
    let cwd = env::current_dir()?;
    let staging_path = cwd.join("opossum_gui").join("staging");
    let examples_target_dir = cwd.join("opossum_gui").join("opm_examples");

    let _staging_guard = StagingGuard {
        path: staging_path.clone(),
    };
    let _examples_guard = ExamplesGuard {
        path: examples_target_dir.clone(),
    };
    {
        println!("\n🚚 Staging binaries...");
        if !staging_path.exists() {
            fs::create_dir_all(&staging_path)?;
        }
        let target_dir = env::var("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Path::new("target").to_path_buf());
        let release_dir = target_dir.join("release");

        let exe_ext = env::consts::EXE_SUFFIX; // ".exe" oder ""

        for bin_name in ["opossum_backend", "opossum_cli"] {
            let src = release_dir.join(format!("{}{}", bin_name, exe_ext));

            // Konstruiert den exakten Namen, den Dioxus' format!("{bin}-{target}") erwartet
            let dest_filename = if exe_ext.is_empty() {
                // Linux / macOS: opossum_backend-x86_64-unknown-linux-gnu
                format!("{}-{}", bin_name, target_triple)
            } else {
                // Windows: opossum_backend.exe-x86_64-pc-windows-msvc
                // THIS IS A WORKAROUND FOR A DIOXUS BUG!!!
                format!("{}{}-{}", bin_name, exe_ext, target_triple)
            };

            let dest = staging_path.join(&dest_filename);

            if src.exists() {
                fs::copy(&src, &dest)?;
                println!("   -> Staged for Dioxus: {}", dest_filename);
            } else {
                return Err(anyhow::anyhow!("Binary not found: {}", src.display()));
            }
        }
    }
    // 4. Generate Examples
    {
        println!("\n📚 Generating example files...");

        if !examples_target_dir.exists() {
            fs::create_dir_all(&examples_target_dir)?;
        }

        let examples = vec![
            "workshop_00_kepler_paraxial",
            "workshop_01_kepler_real_lenses",
            "workshop_02_kepler_chromatism",
            "workshop_03_kepler_wavefront",
            "workshop_04_kepler_imaging_point",
            "workshop_06_geometry_mirrors",
            "workshop_07_geometry_shifted_lens",
            "workshop_08_reference_node",
            "workshop_09_phelix",
            "workshop_10_multi_path",
            "workshop_11_ghostfocus",
        ];

        let _env_guard = sh.push_env(
            "OPOSSUM_EXAMPLES_OUT_DIR",
            examples_target_dir.to_str().unwrap(),
        );

        for example in examples {
            println!("   -> Generating {}...", example);
            cmd!(
                sh,
                "cargo run --release -p opossum_core --example {example}"
            )
            .run()?;
        }
    }
    // 5. Bundling
    {
        println!("\n🎨 Running Dioxus Bundle...");
        let _dir = sh.push_dir("opossum_gui");
        let mut bundle_args = Vec::new();

        if cfg!(target_os = "windows") {
            bundle_args.extend(["--package-types", "msi"]);
        } else if cfg!(target_os = "linux") {
            bundle_args.extend(["--package-types", "deb", "--package-types", "appimage"]);
        } else if cfg!(target_os = "macos") {
            bundle_args.extend(["--package-types", "app", "--package-types", "dmg"]);
        }
        cmd!(sh, "dx bundle --release {bundle_args...}").run()?;
    }
    println!("\n✅ Bundle successfully created!");
    Ok(())
}
fn get_host_target_triple() -> Result<String, anyhow::Error> {
    let output = Command::new("rustc").arg("-vV").output()?;
    let stdout = String::from_utf8(output.stdout)?;

    for line in stdout.lines() {
        if line.starts_with("host: ") {
            return Ok(line.trim_start_matches("host: ").to_string());
        }
    }
    Err(anyhow::anyhow!("Could not determine target triple"))
}
