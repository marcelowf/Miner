use crate::coinbase::{build_coinbase, write_varint, CoinbaseParams};
use crate::config::Config;
use crate::gpu::Gpu;
use crate::merkle::{merkle_root, reverse, sha256d};
use crate::rpc::BitcoinRpcClient;
use crate::sha256_host::midstate;
use crate::template::BlockTemplate;
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Snapshot of mining inputs for the current template (rebuilt every refresh).
struct MiningJob {
    height: u32,
    version: i32,
    prev_hash_internal: [u8; 32],
    bits: [u8; 4],
    target: [u8; 32], // big-endian (display order)
    curtime: u32,
    coinbase_value: u64,
    payout_script: Vec<u8>,
    witness_commitment_script: Option<Vec<u8>>,
    coinbase_tag: Vec<u8>,
    other_txids_internal: Vec<[u8; 32]>,
    other_tx_bytes: Vec<Vec<u8>>,
}

impl MiningJob {
    fn from_template(tpl: BlockTemplate, payout_script: Vec<u8>, coinbase_tag: Vec<u8>) -> Self {
        let prev_hash_internal = reverse(&tpl.previous_blockhash);
        let mut bits = tpl.bits;
        bits.reverse(); // template gives big-endian; header field is little-endian

        let other_txids_internal: Vec<[u8; 32]> =
            tpl.transactions.iter().map(|t| reverse(&t.txid)).collect();
        let other_tx_bytes: Vec<Vec<u8>> =
            tpl.transactions.iter().map(|t| t.data.clone()).collect();

        MiningJob {
            height: tpl.height,
            version: tpl.version,
            prev_hash_internal,
            bits,
            target: tpl.target,
            curtime: tpl.curtime,
            coinbase_value: tpl.coinbase_value,
            payout_script,
            witness_commitment_script: tpl.default_witness_commitment,
            coinbase_tag,
            other_txids_internal,
            other_tx_bytes,
        }
    }
}

/// Everything the GPU needs for one (job, extranonce) pair, plus the data to assemble the block.
struct Prep {
    header: [u8; 80],       // bytes 0..76 fixed; 76..80 (nonce) filled per hit
    midstate: [u32; 8],     // SHA-256 state after header[0..64]
    tail: [u32; 3],         // header[64..76] packed big-endian
    target_words: [u32; 8], // job target as 8 big-endian words (zeroed in benchmark mode)
    coinbase_full: Vec<u8>, // full coinbase serialization for block assembly
}

fn prepare(job: &MiningJob, extranonce: u64, benchmark: bool) -> Prep {
    let coinbase = build_coinbase(&CoinbaseParams {
        height: job.height,
        coinbase_value: job.coinbase_value,
        payout_script: &job.payout_script,
        witness_commitment_script: job.witness_commitment_script.as_deref(),
        extranonce,
        tag: &job.coinbase_tag,
    });

    let mut leaves: Vec<[u8; 32]> = Vec::with_capacity(1 + job.other_txids_internal.len());
    leaves.push(coinbase.txid);
    leaves.extend_from_slice(&job.other_txids_internal);
    let mroot = merkle_root(&leaves);

    let mut header = [0u8; 80];
    header[0..4].copy_from_slice(&job.version.to_le_bytes());
    header[4..36].copy_from_slice(&job.prev_hash_internal);
    header[36..68].copy_from_slice(&mroot);
    header[68..72].copy_from_slice(&job.curtime.to_le_bytes());
    header[72..76].copy_from_slice(&job.bits);
    // header[76..80] (nonce) stays zero; the GPU varies it.

    let mut first64 = [0u8; 64];
    first64.copy_from_slice(&header[0..64]);
    let midstate = midstate(&first64);

    let tail = [
        u32::from_be_bytes([header[64], header[65], header[66], header[67]]),
        u32::from_be_bytes([header[68], header[69], header[70], header[71]]),
        u32::from_be_bytes([header[72], header[73], header[74], header[75]]),
    ];

    // Target as 8 big-endian words (target_words[0] most significant). In benchmark mode we use an
    // impossible (all-zero) target so the kernel never reports a hit — that removes atomic
    // contention and measures pure hashing throughput, independent of the node's real difficulty.
    let mut target_words = [0u32; 8];
    if !benchmark {
        for k in 0..8 {
            target_words[k] = u32::from_be_bytes([
                job.target[4 * k],
                job.target[4 * k + 1],
                job.target[4 * k + 2],
                job.target[4 * k + 3],
            ]);
        }
    }

    Prep {
        header,
        midstate,
        tail,
        target_words,
        coinbase_full: coinbase.full_serialization,
    }
}

pub fn run(cfg: Config, benchmark: bool) -> Result<()> {
    let rpc = BitcoinRpcClient::new(&cfg.rpc.url, &cfg.rpc.user, &cfg.rpc.password);

    let info = rpc
        .get_blockchain_info()
        .context("RPC connection failed (is bitcoind running?)")?;
    let chain = info.get("chain").and_then(|x| x.as_str()).unwrap_or("?");
    let height = info.get("blocks").and_then(|x| x.as_u64()).unwrap_or(0);
    log::info!("Connected to node — chain={chain}, height={height}");

    let payout_script = hex::decode(&cfg.mining.payout_script_hex)?;
    let coinbase_tag = cfg.mining.coinbase_tag.as_bytes().to_vec();

    let mut gpu = Gpu::new(
        cfg.gpu.device_index,
        cfg.gpu.blocks,
        cfg.gpu.threads_per_block,
        cfg.gpu.nonces_per_thread,
    )?;
    let span = gpu.batch_span();
    log::info!(
        "GPU grid: {} blocks × {} threads × {} nonces = {} nonces/launch",
        cfg.gpu.blocks, cfg.gpu.threads_per_block, cfg.gpu.nonces_per_thread, span
    );
    if benchmark {
        log::warn!("BENCHMARK mode: not submitting blocks, measuring raw hashrate only.");
    }

    let initial_tpl = BlockTemplate::from_json(&rpc.get_block_template()?)?;
    let mut job = MiningJob::from_template(initial_tpl, payout_script.clone(), coinbase_tag.clone());
    log::info!(
        "Initial template: height={}, txs={}, value={} sat",
        job.height, job.other_txids_internal.len(), job.coinbase_value
    );

    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc::set_handler(move || {
            log::warn!("Stop requested — finishing current batch...");
            stop.store(true, Ordering::Relaxed);
        })?;
    }

    let refresh = Duration::from_secs(cfg.mining.refresh_seconds.max(1));
    let mut total_hashes: u64 = 0;
    let mut last_hash_count: u64 = 0;
    let mut last_stats = Instant::now();
    let mut last_refresh = Instant::now();
    let mut extranonce: u64 = 0;

    'mining: while !stop.load(Ordering::Relaxed) {
        let prep = prepare(&job, extranonce, benchmark);
        let mut base_nonce: u64 = 0;

        while base_nonce < (1u64 << 32) {
            if stop.load(Ordering::Relaxed) {
                break 'mining;
            }

            let found = gpu.run_batch(&prep.midstate, prep.tail, &prep.target_words, base_nonce as u32)?;
            total_hashes += span;
            base_nonce += span;

            for nonce in found {
                // Re-verify on the host before trusting the GPU result.
                let mut header = prep.header;
                header[76..80].copy_from_slice(&nonce.to_le_bytes());
                let h2 = sha256d(&header);
                if !is_below_target(&h2, &job.target) {
                    log::warn!("GPU reported nonce {nonce} but host re-check failed — ignoring");
                    continue;
                }

                let block_hash_display = reverse(&h2);
                log::info!("★ BLOCK FOUND ★ nonce={nonce} extranonce={extranonce:#x}");
                log::info!("hash = {}", hex::encode(block_hash_display));

                let block_hex = assemble_block_hex(&job, &header, &prep.coinbase_full);
                match rpc.submit_block(&block_hex) {
                    Ok(None) => {
                        log::info!("submitblock OK — you won the lottery. Wait 100 confirmations.");
                        stop.store(true, Ordering::Relaxed);
                        break 'mining;
                    }
                    Ok(Some(reason)) => log::error!("submitblock rejected: {reason}"),
                    Err(e) => log::error!("submitblock failed: {e}"),
                }
            }

            // Hashrate stats — same format as the CPU miner for apples-to-apples comparison.
            if last_stats.elapsed() >= Duration::from_secs(5) {
                let delta = total_hashes - last_hash_count;
                let elapsed = last_stats.elapsed().as_secs_f64();
                let mhs = (delta as f64) / elapsed / 1_000_000.0;
                log::info!("{mhs:>7.2} MH/s | hashes={total_hashes}");
                last_hash_count = total_hashes;
                last_stats = Instant::now();
            }

            // Periodic template refresh (cheap RPC).
            if last_refresh.elapsed() >= refresh / 6 {
                last_refresh = Instant::now();
                match rpc.get_block_template() {
                    Ok(v) => {
                        if let Ok(tpl) = BlockTemplate::from_json(&v) {
                            if tpl.height != job.height || tpl.curtime > job.curtime {
                                log::info!(
                                    "New template: height={}, txs={}",
                                    tpl.height, tpl.transactions.len()
                                );
                                job = MiningJob::from_template(
                                    tpl,
                                    payout_script.clone(),
                                    coinbase_tag.clone(),
                                );
                                extranonce = 0;
                                continue 'mining; // rebuild prep with the new job
                            }
                        }
                    }
                    Err(e) => log::warn!("Template refresh failed: {e}"),
                }
            }
        }

        // Exhausted the 2^32 nonce space for this extranonce — change it to get a fresh search space.
        extranonce += 1;
    }

    log::info!("Stopped. total hashes={total_hashes}");
    Ok(())
}

/// `hash` is little-endian internal sha256d output (32 bytes).
/// `target` is the big-endian display-order target from RPC.
/// Block is valid when reverse(hash) < target lexicographically.
fn is_below_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    for i in 0..32 {
        let h = hash[31 - i];
        let t = target[i];
        if h < t {
            return true;
        }
        if h > t {
            return false;
        }
    }
    false
}

fn assemble_block_hex(job: &MiningJob, header: &[u8; 80], coinbase_full: &[u8]) -> String {
    let mut block = Vec::with_capacity(
        80 + 9 + coinbase_full.len() + job.other_tx_bytes.iter().map(|t| t.len()).sum::<usize>(),
    );
    block.extend_from_slice(header);
    let tx_count = 1 + job.other_tx_bytes.len();
    write_varint(&mut block, tx_count as u64);
    block.extend_from_slice(coinbase_full);
    for tx in &job.other_tx_bytes {
        block.extend_from_slice(tx);
    }
    hex::encode(block)
}
