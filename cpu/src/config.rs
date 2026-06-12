use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub rpc: RpcConfig,
    pub mining: MiningConfig,
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

    /// Number of mining threads. 0 = use all logical cores.
    #[serde(default)]
    pub threads: usize,

    /// Seconds between template refreshes. Lower = waste fewer cycles on stale work
    /// when a competing block lands, higher = less RPC load.
    #[serde(default = "default_refresh")]
    pub refresh_seconds: u64,

    /// Optional tag bytes injected into the coinbase scriptSig after the BIP34 height.
    /// Limited to ~80 bytes when combined with height + extranonce.
    #[serde(default = "default_tag")]
    pub coinbase_tag: String,
}

fn default_refresh() -> u64 { 30 }
fn default_tag() -> String { "/btc-lottery-miner/".to_string() }

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
        Ok(())
    }

    pub fn resolved_threads(&self) -> usize {
        if self.mining.threads == 0 {
            num_cpus::get()
        } else {
            self.mining.threads
        }
    }
}
