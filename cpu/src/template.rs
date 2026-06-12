use anyhow::{anyhow, Context, Result};
use serde_json::Value;

/// Decoded `getblocktemplate` response — the fields we actually need.
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    pub version: i32,
    pub previous_blockhash: [u8; 32], // big-endian as returned by RPC (we reverse before hashing)
    pub bits: [u8; 4],                // big-endian
    pub curtime: u32,
    pub height: u32,
    pub coinbase_value: u64,
    pub target: [u8; 32], // big-endian
    pub default_witness_commitment: Option<Vec<u8>>, // scriptPubKey for witness commitment, if any segwit txs
    pub transactions: Vec<TemplateTx>,
}

#[derive(Debug, Clone)]
pub struct TemplateTx {
    pub data: Vec<u8>,       // full serialized tx (with witness if any)
    pub txid: [u8; 32],      // big-endian as returned (we reverse for merkle input)
    pub hash: [u8; 32],      // big-endian; equals txid for non-witness, equals wtxid for witness txs
    pub fee: u64,
    pub weight: u64,
}

impl BlockTemplate {
    pub fn from_json(v: &Value) -> Result<Self> {
        let version = v.get("version").and_then(|x| x.as_i64())
            .ok_or_else(|| anyhow!("missing version"))? as i32;

        let previous_blockhash = parse_hash32(v, "previousblockhash")?;
        let curtime = v.get("curtime").and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow!("missing curtime"))? as u32;
        let height = v.get("height").and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow!("missing height"))? as u32;
        let coinbase_value = v.get("coinbasevalue").and_then(|x| x.as_u64())
            .ok_or_else(|| anyhow!("missing coinbasevalue"))?;

        let bits_str = v.get("bits").and_then(|x| x.as_str())
            .ok_or_else(|| anyhow!("missing bits"))?;
        let bits_vec = hex::decode(bits_str).context("invalid hex in bits")?;
        if bits_vec.len() != 4 { return Err(anyhow!("bits must be 4 bytes")); }
        let mut bits = [0u8; 4];
        bits.copy_from_slice(&bits_vec);

        let target = parse_hash32(v, "target")?;

        let default_witness_commitment = v
            .get("default_witness_commitment")
            .and_then(|x| x.as_str())
            .map(|s| hex::decode(s).context("invalid hex in default_witness_commitment"))
            .transpose()?;

        let txs = v.get("transactions").and_then(|x| x.as_array())
            .ok_or_else(|| anyhow!("missing transactions array"))?;

        let mut transactions = Vec::with_capacity(txs.len());
        for t in txs {
            let data_hex = t.get("data").and_then(|x| x.as_str())
                .ok_or_else(|| anyhow!("tx missing data"))?;
            let data = hex::decode(data_hex).context("invalid tx data hex")?;

            let txid = parse_hash32(t, "txid")?;
            let hash = t.get("hash").and_then(|x| x.as_str())
                .map(parse_hex32)
                .transpose()?
                .unwrap_or(txid);

            let fee = t.get("fee").and_then(|x| x.as_u64()).unwrap_or(0);
            let weight = t.get("weight").and_then(|x| x.as_u64()).unwrap_or(0);

            transactions.push(TemplateTx { data, txid, hash, fee, weight });
        }

        Ok(BlockTemplate {
            version,
            previous_blockhash,
            bits,
            curtime,
            height,
            coinbase_value,
            target,
            default_witness_commitment,
            transactions,
        })
    }
}

fn parse_hash32(v: &Value, field: &str) -> Result<[u8; 32]> {
    let s = v.get(field).and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("missing {field}"))?;
    parse_hex32(s).with_context(|| format!("invalid hex in {field}"))
}

fn parse_hex32(s: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(s)?;
    if bytes.len() != 32 {
        return Err(anyhow!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}
