use crate::coinbase::{build_coinbase, write_varint, CoinbaseParams};
use crate::config::Config;
use crate::merkle::{merkle_root, reverse, sha256d};
use crate::rpc::BitcoinRpcClient;
use crate::template::BlockTemplate;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Snapshot of mining inputs shared by all workers for the current template.
/// Rebuilt every refresh; workers atomically swap their reference to it.
struct MiningJob {
    height: u32,
    version: i32,
    prev_hash_internal: [u8; 32], // already reversed for header use
    bits: [u8; 4],                // already reversed for header use
    target: [u8; 32],             // big-endian (display); we compare hash-as-big-endian < target
    curtime: u32,
    coinbase_value: u64,
    payout_script: Vec<u8>,
    witness_commitment_script: Option<Vec<u8>>,
    coinbase_tag: Vec<u8>,
    /// txids of NON-coinbase transactions, in internal byte order (for merkle).
    other_txids_internal: Vec<[u8; 32]>,
    /// Raw serialized tx bytes (with witness if any) for assembling the final block.
    other_tx_bytes: Vec<Vec<u8>>,
}

impl MiningJob {
    fn from_template(tpl: BlockTemplate, payout_script: Vec<u8>, coinbase_tag: Vec<u8>) -> Self {
        let prev_hash_internal = reverse(&tpl.previous_blockhash);
        let mut bits = tpl.bits;
        bits.reverse(); // template gives big-endian; header field is little-endian

        let other_txids_internal: Vec<[u8; 32]> = tpl
            .transactions
            .iter()
            .map(|t| reverse(&t.txid))
            .collect();
        let other_tx_bytes: Vec<Vec<u8>> = tpl.transactions.iter().map(|t| t.data.clone()).collect();

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

/// Shared mining state — workers swap the Arc<MiningJob> when the template refreshes.
pub struct MiningState {
    job: arc_swap::ArcSwap<MiningJob>,
    stop: AtomicBool,
    hashes: AtomicU64,
}

mod arc_swap {
    // Tiny inline arc-swap to avoid pulling in another crate.
    use std::sync::{Arc, Mutex};
    pub struct ArcSwap<T> {
        inner: Mutex<Arc<T>>,
    }
    impl<T> ArcSwap<T> {
        pub fn new(v: Arc<T>) -> Self { Self { inner: Mutex::new(v) } }
        pub fn load(&self) -> Arc<T> { self.inner.lock().unwrap().clone() }
        pub fn store(&self, v: Arc<T>) { *self.inner.lock().unwrap() = v; }
    }
}

pub fn run(cfg: Config) -> Result<()> {
    let rpc = Arc::new(BitcoinRpcClient::new(
        &cfg.rpc.url,
        &cfg.rpc.user,
        &cfg.rpc.password,
    ));

    // Sanity check the connection
    let info = rpc.get_blockchain_info().context("RPC connection failed (is bitcoind running?)")?;
    let chain = info.get("chain").and_then(|x| x.as_str()).unwrap_or("?");
    let height = info.get("blocks").and_then(|x| x.as_u64()).unwrap_or(0);
    log::info!("Connected to node — chain={chain}, height={height}");

    let payout_script = hex::decode(&cfg.mining.payout_script_hex)?;
    let coinbase_tag = cfg.mining.coinbase_tag.as_bytes().to_vec();

    // Initial template
    let initial_tpl = BlockTemplate::from_json(&rpc.get_block_template()?)?;
    let initial_height = initial_tpl.height;
    let job = Arc::new(MiningJob::from_template(initial_tpl, payout_script.clone(), coinbase_tag.clone()));
    log::info!(
        "Initial template: height={}, txs={}, value={} sat",
        job.height, job.other_txids_internal.len(), job.coinbase_value
    );

    let state = Arc::new(MiningState {
        job: arc_swap::ArcSwap::new(job),
        stop: AtomicBool::new(false),
        hashes: AtomicU64::new(0),
    });

    // Ctrl+C → graceful stop
    {
        let state = state.clone();
        ctrlc::set_handler(move || {
            log::warn!("Stop requested — finishing current iteration...");
            state.stop.store(true, Ordering::Relaxed);
        })?;
    }

    // Spawn workers
    let n_threads = cfg.resolved_threads();
    log::info!("Mining with {n_threads} threads");
    let mut handles = Vec::with_capacity(n_threads);
    for thread_id in 0..n_threads {
        let state = state.clone();
        let rpc = rpc.clone();
        handles.push(thread::spawn(move || worker_loop(thread_id, n_threads, state, rpc)));
    }

    // Template refresh loop + stats
    let refresh = Duration::from_secs(cfg.mining.refresh_seconds.max(1));
    let mut last_stats = Instant::now();
    let mut last_hash_count = 0u64;
    let mut current_height = initial_height;

    while !state.stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));

        if last_stats.elapsed() >= Duration::from_secs(5) {
            let total = state.hashes.load(Ordering::Relaxed);
            let delta = total - last_hash_count;
            let elapsed = last_stats.elapsed().as_secs_f64();
            let mhs = (delta as f64) / elapsed / 1_000_000.0;
            log::info!("{mhs:>7.2} MH/s | hashes={total}");
            last_hash_count = total;
            last_stats = Instant::now();
        }

        if last_stats.elapsed() >= refresh / 6 {
            // periodic poll for new template (cheap RPC)
            match rpc.get_block_template() {
                Ok(v) => {
                    if let Ok(tpl) = BlockTemplate::from_json(&v) {
                        if tpl.height != current_height || tpl.curtime > state.job.load().curtime {
                            current_height = tpl.height;
                            let new_job = Arc::new(MiningJob::from_template(
                                tpl,
                                payout_script.clone(),
                                coinbase_tag.clone(),
                            ));
                            log::info!(
                                "New template: height={}, txs={}",
                                new_job.height, new_job.other_txids_internal.len()
                            );
                            state.job.store(new_job);
                        }
                    }
                }
                Err(e) => log::warn!("Template refresh failed: {e}"),
            }
        }
    }

    log::info!("Stopping workers...");
    state.stop.store(true, Ordering::Relaxed);
    for h in handles {
        let _ = h.join();
    }
    Ok(())
}

fn worker_loop(
    thread_id: usize,
    n_threads: usize,
    state: Arc<MiningState>,
    rpc: Arc<BitcoinRpcClient>,
) {
    // Each worker partitions the search space via its extranonce.
    // We treat extranonce as `(thread_id, local_counter)` so threads never collide.
    let mut local_extra: u32 = 0;
    let mut current_job_ptr: usize = 0;

    while !state.stop.load(Ordering::Relaxed) {
        let job = state.job.load();
        let job_ptr = Arc::as_ptr(&job) as usize;
        if job_ptr != current_job_ptr {
            current_job_ptr = job_ptr;
            local_extra = 0;
        }

        // Compose extranonce: high 32 bits = thread_id, low 32 = local counter
        let extranonce = ((thread_id as u64) << 32) | (local_extra as u64);
        local_extra = local_extra.wrapping_add(1);

        // Build coinbase + merkle root for this extranonce
        let coinbase = build_coinbase(&CoinbaseParams {
            height: job.height,
            coinbase_value: job.coinbase_value,
            payout_script: &job.payout_script,
            witness_commitment_script: job.witness_commitment_script.as_deref(),
            extranonce,
            tag: &job.coinbase_tag,
        });

        // Merkle leaves = [coinbase_txid_internal, then each other tx's txid_internal]
        // The template provides txids in display order; we reversed them at job build time.
        let coinbase_txid_internal = coinbase.txid; // sha256d already in internal order
        let mut leaves: Vec<[u8; 32]> = Vec::with_capacity(1 + job.other_txids_internal.len());
        leaves.push(coinbase_txid_internal);
        leaves.extend_from_slice(&job.other_txids_internal);
        let mroot = merkle_root(&leaves);

        // Build 80-byte header skeleton (last 4 bytes = nonce, varied in the inner loop)
        let mut header = [0u8; 80];
        header[0..4].copy_from_slice(&job.version.to_le_bytes());
        header[4..36].copy_from_slice(&job.prev_hash_internal);
        header[36..68].copy_from_slice(&mroot);
        header[68..72].copy_from_slice(&job.curtime.to_le_bytes());
        header[72..76].copy_from_slice(&job.bits);

        // Inner loop: vary nonce 0..2^32 for this (extranonce, merkle_root) combo
        let mut nonce: u32 = 0;
        let mut local_count: u64 = 0;
        loop {
            if state.stop.load(Ordering::Relaxed) { return; }
            if local_count & 0xFFFF == 0 {
                // Cheap check: did the global job pointer change? (new template arrived)
                if Arc::as_ptr(&state.job.load()) as usize != current_job_ptr {
                    break;
                }
                // Push hash counter to global stats every 65536 hashes
                state.hashes.fetch_add(local_count, Ordering::Relaxed);
                local_count = 0;
            }

            header[76..80].copy_from_slice(&nonce.to_le_bytes());

            let h1 = Sha256::digest(header);
            let h2 = Sha256::digest(h1);
            // h2 is little-endian internal hash. Bitcoin display order is reversed.
            // Target comparison is done on the reversed (display-order, big-endian) hash.
            if is_below_target(&h2, &job.target) {
                state.hashes.fetch_add(local_count, Ordering::Relaxed);
                log::info!(
                    "★ BLOCK FOUND ★ thread={thread_id} nonce={nonce} extranonce={extranonce:#x}"
                );
                let mut h2_arr = [0u8; 32];
                h2_arr.copy_from_slice(&h2);
                let block_hash_display = reverse(&h2_arr);
                log::info!("hash = {}", hex::encode(block_hash_display));

                // Assemble and submit
                let block_hex = assemble_block_hex(&job, &header, &coinbase.full_serialization);
                match rpc.submit_block(&block_hex) {
                    Ok(None) => {
                        log::info!("submitblock OK — you won the lottery. Wait 100 confirmations.");
                        state.stop.store(true, Ordering::Relaxed);
                        return;
                    }
                    Ok(Some(reason)) => log::error!("submitblock rejected: {reason}"),
                    Err(e) => log::error!("submitblock failed: {e}"),
                }
                break; // refresh template
            }

            local_count += 1;
            if nonce == u32::MAX { break; }
            nonce += 1;
        }

        state.hashes.fetch_add(local_count, Ordering::Relaxed);
        let _ = n_threads; // silence unused if we later remove partitioning math
    }
}

/// `hash` is little-endian internal sha256d output (32 bytes).
/// `target` is the big-endian display-order target from RPC.
/// Block is valid when reverse(hash) < target lexicographically.
fn is_below_target(hash: &[u8], target: &[u8; 32]) -> bool {
    // reverse(hash) is equivalent to comparing hash bytes from index 31 down vs target 0..31.
    for i in 0..32 {
        let h = hash[31 - i];
        let t = target[i];
        if h < t { return true; }
        if h > t { return false; }
    }
    false
}

fn assemble_block_hex(job: &MiningJob, header: &[u8; 80], coinbase_full: &[u8]) -> String {
    let mut block = Vec::with_capacity(80 + 9 + coinbase_full.len() + job.other_tx_bytes.iter().map(|t| t.len()).sum::<usize>());
    block.extend_from_slice(header);
    let tx_count = 1 + job.other_tx_bytes.len();
    write_varint(&mut block, tx_count as u64);
    block.extend_from_slice(coinbase_full);
    for tx in &job.other_tx_bytes {
        block.extend_from_slice(tx);
    }
    hex::encode(block)
}

/// Exposed for potential future use (e.g. debugging or precomputing midstates).
#[allow(dead_code)]
pub fn header_hash(header: &[u8; 80]) -> [u8; 32] {
    sha256d(header)
}
