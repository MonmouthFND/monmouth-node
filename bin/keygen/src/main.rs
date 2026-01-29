#![doc = "Key generation and setup tool for Kora devnet."]

use clap::Parser;
use eyre::Result;
use tracing_subscriber::{EnvFilter, fmt};

mod setup;

#[derive(Parser, Debug)]
#[command(name = "keygen")]
#[command(about = "Key generation and setup tool for Kora devnet")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    Setup(setup::SetupArgs),
}

fn main() -> Result<()> {
    fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup(args) => setup::run(args),
    }
}
