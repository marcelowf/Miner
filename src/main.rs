mod coinbase;
mod config;
mod merkle;
mod mining;
mod rpc;
mod template;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "btc-lottery-miner",
    version,
    about = "Solo CPU Bitcoin miner — lottery edition"
)]
struct Cli {
    /// Path to config TOML file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Override log level: error, warn, info, debug, trace
    #[arg(long, default_value = "info")]
    log: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log))
        .format_timestamp_secs()
        .init();

    log::info!("btc-lottery-miner v{}", env!("CARGO_PKG_VERSION"));
    let cfg = config::Config::load(&cli.config)?;
    mining::run(cfg)
}
