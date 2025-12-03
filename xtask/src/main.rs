use clap::{Parser, Subcommand};
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

fn task_bundle() -> Result<(), anyhow::Error> {
    let sh = Shell::new()?;

    println!("🦀 Start build process for OPOSSUM...");

    // 1. Build opossum_cli
    {
        println!("\n📦 Build opossum_cli (Release)...");
        cmd!(sh, "cargo build --release -p opossum_cli").run()?;
    }

    // 2. Build opossum_backend
    {
        println!("\n📦 Build opossum_backend (Release)...");
        cmd!(sh, "cargo build --release -p opossum_backend").run()?;
    }

    // 3. Bundle opossum_gui
    {
        println!("\n🎨 Build & Bundle opossum_gui...");
        let _dir = sh.push_dir("opossum_gui");
        cmd!(sh, "dx bundle --release --features bundle-backend").run()?;
    }

    println!("\n✅ Bundle sucessfully created!");
    Ok(())
}
