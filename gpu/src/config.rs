use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub rpc: RpcConfig,
    pub mining: MiningConfig,
    #[serde(default)]
    pub gpu: GpuConfig,
}

#[derive(Debug, Deserialize)]
pub struct RpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct MiningConfig {
    /// Hex-encoded scriptPubKey for the coinbase output (your payout).
    /// Get with: bitcoin-cli getaddressinfo $(bitcoin-cli getnewaddress) | jq -r .scriptPubKey
    pub payout_script_hex: String,

    /// Seconds between template refreshes. Lower = waste fewer cycles on stale work
    /// when a competing block lands, higher = less RPC load.
    #[serde(default = "default_refresh")]
    pub refresh_seconds: u64,

    /// Optional tag bytes injected into the coinbase scriptSig after the BIP34 height.
    /// Limited to ~80 bytes when combined with height + extranonce.
    #[serde(default = "default_tag")]
    pub coinbase_tag: String,
}

/// GPU dispatch tuning. The kernel is launched as a grid of
/// `blocks * threads_per_block` threads, each hashing `nonces_per_thread` nonces.
/// Total nonces per kernel launch = blocks * threads_per_block * nonces_per_thread.
#[derive(Debug, Deserialize)]
pub struct GpuConfig {
    /// Which CUDA device to use (0 = first GPU).
    #[serde(default)]
    pub device_index: usize,

    /// CUDA threads per block. 256 is a safe default for most GPUs.
    #[serde(default = "default_tpb")]
    pub threads_per_block: u32,

    /// Number of CUDA blocks per launch ("intensity"). Raise until the GPU saturates
    /// without freezing the desktop; lower it if the screen gets laggy.
    #[serde(default = "default_blocks")]
    pub blocks: u32,

    /// Nonces hashed per thread per launch. Bigger = fewer launches, less host overhead.
    #[serde(default = "default_npt")]
    pub nonces_per_thread: u32,
}

impl Default for GpuConfig {
    fn default() -> Self {
        GpuConfig {
            device_index: 0,
            threads_per_block: default_tpb(),
            blocks: default_blocks(),
            nonces_per_thread: default_npt(),
        }
    }
}

fn default_refresh() -> u64 { 30 }
fn default_tag() -> String { "/btc-lottery-miner-gpu/".to_string() }
fn default_tpb() -> u32 { 256 }
fn default_blocks() -> u32 { 4096 }
fn default_npt() -> u32 { 64 }

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let cfg: Config = toml::from_str(&raw).context("failed to parse config TOML")?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        hex::decode(&self.mining.payout_script_hex)
            .context("mining.payout_script_hex is not valid hex")?;
        if self.mining.payout_script_hex.is_empty() {
            anyhow::bail!("mining.payout_script_hex must not be empty");
        }
        if !self.rpc.url.starts_with("http") {
            anyhow::bail!("rpc.url must be an http(s) URL");
        }
        if self.gpu.threads_per_block == 0 || self.gpu.blocks == 0 || self.gpu.nonces_per_thread == 0 {
            anyhow::bail!("gpu.threads_per_block, gpu.blocks and gpu.nonces_per_thread must all be > 0");
        }
        Ok(())
    }
}
