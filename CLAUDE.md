# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

`btc-lottery-miner` — a solo CPU Bitcoin miner ("lottery" miner) written in Rust. It connects to a
Bitcoin Core node via JSON-RPC, fetches a block template (`getblocktemplate`), builds an 80-byte
block header plus a coinbase transaction, brute-forces nonces with SHA256d, and submits any valid
block via `submitblock`. It's an educational/lottery project — see `README.md` for the (very long)
odds of finding a block on mainnet.

## Build & run commands

- Release build: `cargo build --release` → binary at `target\release\btc-lottery-miner.exe`
- Debug build: `cargo build`
- Run: `.\target\release\btc-lottery-miner.exe --config config.toml`
  (or `cargo run --release -- --config config.toml`)
- Log level override: `--log debug` (`error|warn|info|debug|trace`)
- Lint/typecheck: `cargo check`, `cargo clippy`
- There is no test suite in this repo (no `#[cfg(test)]` modules).

## Configuration

- `config.toml` is gitignored (contains real RPC credentials and payout script). Copy from
  `config.example.toml` to create it.
- `[rpc]`: `url`/`user`/`password` for the Bitcoin Core JSON-RPC endpoint. Default ports:
  regtest `18443`, testnet4 `48332`, mainnet `8332`.
- `[mining]`:
  - `payout_script_hex` — the coinbase output **scriptPubKey**, not a bech32 address. Derive with
    `bitcoin-cli getaddressinfo <addr> | jq -r .scriptPubKey`.
  - `threads` — `0` = use all logical cores (`Config::resolved_threads`).
  - `refresh_seconds` — how often the main loop polls `getblocktemplate` for new work.
  - `coinbase_tag` — free-form bytes appended to the coinbase scriptSig; total scriptSig must stay
    `<= 100` bytes per consensus (enforced in `coinbase.rs` by clamping tag length).

## Architecture

Module flow: `main.rs` → `config::Config::load` → `mining::run`.

- **`rpc.rs`** — `BitcoinRpcClient`: thin JSON-RPC wrapper (HTTP basic auth via `ureq`) exposing
  `get_block_template`, `submit_block`, `get_blockchain_info`.
- **`template.rs`** — `BlockTemplate::from_json`: parses a `getblocktemplate` response into typed
  fields (version, `previous_blockhash`, `bits`, `target`, `coinbase_value`,
  `default_witness_commitment`, and the list of `TemplateTx` with raw tx bytes + txids).
- **`coinbase.rs`** — `build_coinbase`: constructs the coinbase transaction (BIP34 height push +
  per-thread extranonce push + `coinbase_tag`, payout output, optional witness-commitment output).
  Returns both a witness-stripped serialization (used for the txid that feeds the merkle root) and
  a full serialization (used in the assembled block). `push_bip34_height` is consensus-critical:
  heights 1–16 must be encoded as a single `OP_1`..`OP_16` byte (`0x51`-`0x60`), not a CScriptNum —
  see `docs/regtest.md` for the history of the `bad-cb-height` bug this fixes.
- **`merkle.rs`** — `sha256d` / `sha256d_pair` / `merkle_root`, plus `reverse` (32-byte order flip).
  Bitcoin RPC returns hashes in display (reversed) byte order; header hashing needs internal
  (protocol) order. `reverse()` converts between the two and is used throughout — see "Byte-order
  conventions" below.
- **`mining.rs`** — the core loop:
  - `MiningJob` — immutable snapshot of everything needed to mine one template, built once per
    refresh via `MiningJob::from_template`.
  - `MiningState` — shared across worker threads: the current `MiningJob` behind a tiny inline
    `arc_swap::ArcSwap` (a `Mutex<Arc<T>>`, not the `arc-swap` crate), an atomic stop flag, and a
    global hash counter.
  - `run()` — connects via RPC, builds the initial job, installs a Ctrl+C handler, spawns one
    worker thread per `resolved_threads()`, then loops: every 500ms check the stop flag, every 5s
    log the hashrate, every `refresh_seconds/6` poll `getblocktemplate` and swap in a new
    `MiningJob` if the height changed or `curtime` advanced.
  - `worker_loop()` — each thread partitions the search space via
    `extranonce = (thread_id << 32) | local_counter`. Changing the extranonce changes the coinbase
    txid → merkle root → header, giving each thread an independent ~2^32 nonce range. Per nonce:
    fill in the header, `sha256d`, compare against the target with `is_below_target` (walks the
    hash bytes in reverse order against the big-endian target, avoiding an allocation). On a hit,
    assemble the full block hex (`assemble_block_hex`) and call `submit_block`. Every 65536
    iterations it checks whether the shared job pointer changed (new template arrived) and breaks
    out of the inner loop to pick up fresh work.
  - Deliberately no SIMD / midstate caching — optimized for readability (see README "O hot loop").

## Byte-order conventions

Bitcoin RPC fields (`previousblockhash`, `bits`, `target`, `txid`) are given in **display order**
(reversed from how the protocol hashes them). Header fields and hash comparisons need **internal
(protocol) order**. `reverse()` converts between the two, and field names use an `_internal` suffix
(e.g. `prev_hash_internal`, `other_txids_internal`) to track which order a value is in. When adding
new template/header fields, follow this naming convention to avoid mixing the two orders.

## Docs

`docs/` contains step-by-step Windows setup/run logs per network (in Portuguese):
- `setup.md` — toolchain + Bitcoin Core install on Windows
- `regtest.md` — local regtest walkthrough, including the BIP34 height-encoding bug fix
- `testnet4.md` — testnet4 sync + run log
- `mainnet.md` — mainnet sync, storage, and security/privacy notes
- `banchmarks/PC1/` — hashrate benchmark logs for a reference machine (i7-10700, ~9 MH/s)
