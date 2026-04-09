//! `forge` CLI entrypoint.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "forge", about = "forge-index — EVM indexing framework")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start in development mode with hot reload.
    Dev {
        /// Path to the entrypoint file.
        #[arg(long, default_value = "src/main.rs")]
        entry: PathBuf,
        /// Port for the API server.
        #[arg(long, default_value = "42069")]
        port: u16,
        /// Postgres schema name override.
        #[arg(long)]
        schema: Option<String>,
    },
    /// Start in production mode.
    Start {
        /// Path to the entrypoint file.
        #[arg(long, default_value = "src/main.rs")]
        entry: PathBuf,
        /// Port for the API server.
        #[arg(long, default_value = "42069")]
        port: u16,
        /// Postgres schema name override.
        #[arg(long)]
        schema: Option<String>,
    },
    /// Generate Rust types from an ABI JSON file.
    Codegen {
        /// Path to the ABI JSON file.
        #[arg(long)]
        abi: PathBuf,
        /// Output directory for generated files.
        #[arg(long, default_value = "src/generated")]
        output: PathBuf,
        /// Contract name (e.g. "ERC20").
        #[arg(long)]
        name: String,
    },
    /// Run database migrations manually.
    Migrate {
        /// Database connection URL (overrides DATABASE_URL env var).
        #[arg(long)]
        database_url: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dev {
            entry,
            port,
            schema,
        } => forge_index_cli::commands::dev::run(entry, port, schema),
        Commands::Start {
            entry,
            port,
            schema,
        } => forge_index_cli::commands::start::run(entry, port, schema),
        Commands::Codegen { abi, output, name } => {
            forge_index_cli::commands::codegen::run(abi, output, name)
        }
        Commands::Migrate { database_url } => {
            forge_index_cli::commands::migrate::run(database_url).await
        }
    }
}
