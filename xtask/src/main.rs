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

/// Ein Wächter, der den Staging-Ordner löscht, wenn er out-of-scope geht.
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
    // 3. Create staging area and rename/copy binaries
    let staging_path = Path::new("opossum_gui/staging").to_path_buf();
    let _guard = StagingGuard {
        path: staging_path.clone(),
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
            // source: target/release/opossum_backend.exe
            let src = release_dir.join(format!("{}{}", bin_name, exe_ext));
            // Destinattion: opossum_gui/staging/opossum_backend-<host-triple><ext>
            let dest_filename = format!("{}-{}{}", bin_name, target_triple, exe_ext);
            let dest = staging_path.join(&dest_filename);

            if src.exists() {
                fs::copy(&src, &dest)?;
                println!("   -> Staged: {}", dest_filename);
            } else {
                return Err(anyhow::anyhow!("Binary not found: {}", src.display()));
            }
        }
    }
    // 4. Bundling
    {
        println!("\n🎨 Running Dioxus Bundle...");
        let _dir = sh.push_dir("opossum_gui");
        cmd!(sh, "dx bundle --release").run()?;
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
