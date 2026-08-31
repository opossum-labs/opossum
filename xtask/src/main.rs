use clap::{Parser, Subcommand};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use xshell::{Shell, cmd};

/// List of all example targets defined in `opossum_core`
const EXAMPLES: &[&str] = &[
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
    "amplifier_const_gain_chain",
    "amplifier_gaussian_pump_fluence",
];

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
    /// Generate example files from `opossum_core`
    Examples {
        /// Target directory where generated example files are placed.
        /// Defaults to `<workspace_root>/opm_examples` if omitted.
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bundle => task_bundle()?,
        Commands::Ci => xtaskops::tasks::ci()?,
        Commands::Coverage => xtaskops::tasks::coverage(true)?,
        Commands::Examples { output_dir } => {
            // If no explicit path is provided, resolve to <workspace_root>/opm_examples
            let target_dir = match output_dir {
                Some(dir) => dir,
                None => project_root()?.join("opm_examples"),
            };
            task_examples(&target_dir)?;
        }
    }
    Ok(())
}

/// Helper function to locate the workspace root directory.
/// Assumes this crate is located at `<workspace_root>/xtask`.
fn project_root() -> Result<PathBuf, anyhow::Error> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Failed to find workspace root directory"))?;
    Ok(root.to_path_buf())
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

/// Standalone task to generate example files
fn task_examples(output_dir: &Path) -> Result<(), anyhow::Error> {
    let sh = Shell::new()?;
    // Always change into the workspace root directory for consistent relative cargo calls
    let root = project_root()?;
    let _dir_guard = sh.push_dir(&root);

    generate_examples(&sh, output_dir)?;
    println!(
        "\n✨ Examples generation completed in '{}'",
        output_dir.display()
    );
    Ok(())
}

/// Helper function to build all example files into a specific target directory
fn generate_examples(sh: &Shell, target_dir: &Path) -> Result<(), anyhow::Error> {
    println!(
        "\n📚 Generating example files into '{}'...",
        target_dir.display()
    );

    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }

    // Canonicalize or ensure we have an absolute path so child processes receive a stable path
    let target_dir_absolute = if target_dir.is_relative() {
        env::current_dir()?.join(target_dir)
    } else {
        target_dir.to_path_buf()
    };

    let target_dir_str = target_dir_absolute
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Target path contains invalid UTF-8 characters"))?;

    // Push the output directory to the environment for child processes
    let _env_guard = sh.push_env("OPOSSUM_EXAMPLES_OUT_DIR", target_dir_str);

    for example in EXAMPLES {
        println!("   -> Generating {example}...");
        // Suppress command echoing and Cargo compilation messages
        cmd!(sh, "cargo run --quiet -p opossum_core --example {example}")
            .quiet()
            .run()?;
    }

    Ok(())
}

fn task_bundle() -> Result<(), anyhow::Error> {
    let sh = Shell::new()?;
    let root = project_root()?;
    let _dir_guard = sh.push_dir(&root);

    println!("🦀 Start build process for OPOSSUM...");

    // 1. Derive host target triple
    let target_triple = get_host_target_triple()?;
    println!("🎯 Target Triple: {target_triple}");

    // 2. Locate Cargo's build output directory (respects external CARGO_TARGET_DIR)
    let cargo_target_dir =
        env::var("CARGO_TARGET_DIR").map_or_else(|_| root.join("target"), PathBuf::from);
    let release_dir = cargo_target_dir.join("release");

    // 3. Define workspace-local target directory for staging and resources
    // This ensures relative paths in Dioxus.toml ('../target/...') remain valid at all times.
    let local_target_dir = root.join("target");
    let staging_path = local_target_dir.join("staging");
    let examples_target_dir = local_target_dir.join("opm_examples");

    // Guards automatically remove directories when exiting task_bundle (or on error)
    let _staging_guard = StagingGuard {
        path: staging_path.clone(),
    };
    let _examples_guard = ExamplesGuard {
        path: examples_target_dir.clone(),
    };

    // 4. Build binaries (Release)
    {
        println!("\n📦 Building binaries...");
        cmd!(sh, "cargo build --release -p opossum_cli").run()?;
        cmd!(sh, "cargo build --release -p opossum_backend").run()?;
    }

    // 5. Stage binaries from Cargo's output dir into the local target staging dir
    {
        println!("\n🚚 Staging binaries into target directory...");
        if !staging_path.exists() {
            fs::create_dir_all(&staging_path)?;
        }

        let exe_ext = env::consts::EXE_SUFFIX; // ".exe" or ""

        for bin_name in ["opossum_backend", "opossum_cli"] {
            let src = release_dir.join(format!("{bin_name}{exe_ext}"));

            // Format filename as expected by Dioxus bundler
            let dest_filename = if exe_ext.is_empty() {
                format!("{bin_name}-{target_triple}")
            } else {
                // Windows workaround for Dioxus bundling
                format!("{bin_name}{exe_ext}-{target_triple}")
            };

            let dest = staging_path.join(&dest_filename);

            if src.exists() {
                fs::copy(&src, &dest)?;
                println!("   -> Staged for Dioxus: {dest_filename}");
            } else {
                return Err(anyhow::anyhow!(
                    "Binary not found at expected build location: {}",
                    src.display()
                ));
            }
        }
    }

    // 6. Generate Examples into local workspace target directory
    generate_examples(&sh, &examples_target_dir)?;

    // 7. Execute Dioxus Bundle
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

    // 8. Windows-specific: Compile final EXE Installer using Inno Setup
    #[cfg(target_os = "windows")]
    {
        println!("\n🛠️ Running Inno Setup Compiler (ISCC) for EXE Installer...");
        cmd!(
            sh,
            "'C:\\Program Files\\Inno Setup 7\\ISCC.exe' .\\installer.iss"
        )
        .run()?;
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
