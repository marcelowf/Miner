mod coinbase;
mod config;
mod gpu;
mod merkle;
mod mining;
mod rpc;
mod sha256_host;
mod template;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "btc-lottery-miner-gpu",
    version,
    about = "Solo GPU Bitcoin miner (CUDA) — lottery edition"
)]
struct Cli {
    /// Path to config TOML file
    #[arg(short, long, default_value = "config.toml")]
    config: PathBuf,

    /// Override log level: error, warn, info, debug, trace
    #[arg(long, default_value = "info")]
    log: String,

    /// List CUDA devices and exit (no node needed).
    #[arg(long)]
    list_devices: bool,

    /// Benchmark mode: hash as fast as possible and never submit blocks. Measures raw GPU
    /// hashrate independent of the node's difficulty (works against a regtest node).
    #[arg(long)]
    benchmark: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log))
        .format_timestamp_secs()
        .init();

    log::info!("btc-lottery-miner-gpu v{}", env!("CARGO_PKG_VERSION"));

    if cli.list_devices {
        return gpu::list_devices();
    }

    let cfg = config::Config::load(&cli.config)?;
    mining::run(cfg, cli.benchmark)
}
