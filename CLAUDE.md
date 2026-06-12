# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A solo Bitcoin "lottery" miner — connects to a Bitcoin Core node via JSON-RPC, builds an 80-byte
block header, and searches for a nonce where `SHA256(SHA256(header)) < target`. Two independent
implementations live side by side under [Miner/](Miner/) (the git repo root is `Miner/`, not
`c:\Develop`):

- [Miner/cpu/](Miner/cpu/) — CPU miner (`btc-lottery-miner`), multi-threaded. **Stable; do not change** unless the task is about the CPU miner.
- [Miner/gpu/](Miner/gpu/) — CUDA GPU miner (`btc-lottery-miner-gpu`), targets a GTX 1650. Newer; the active work (see [todo.md](todo.md)).

Both are Rust crates with their own `Cargo.toml`; there is **no workspace** at the `Miner/` root.
Docs/prose are in Portuguese; code and identifiers are in English.

## Commands

Build/test/run per crate — `cd` into `Miner/cpu` or `Miner/gpu` first (no root manifest to build
the whole repo at once, despite what `docs/setup.md` §5 implies):

```powershell
cd Miner/gpu                       # or Miner/cpu
cargo build --release              # binary: target\release\btc-lottery-miner-gpu.exe
cargo test                         # SHA-256 midstate tests live in gpu/src/sha256_host.rs
cargo test midstate                # run a single test by name substring
```

GPU-only:

```powershell
.\target\release\btc-lottery-miner-gpu.exe --list-devices         # enumerate CUDA GPUs, no node needed
.\target\release\btc-lottery-miner-gpu.exe --config config.toml              # mine
.\target\release\btc-lottery-miner-gpu.exe --config config.toml --benchmark  # measure raw hashrate, never submits
```

Both binaries take `--config <path>` (default `config.toml`) and `--log <error|warn|info|debug|trace>`.
Copy `config.example.toml` → `config.toml` in the crate dir before running. The GPU `--benchmark`
flag zeroes the target so the kernel reports no hits — measures pure throughput against any node,
including a throwaway `regtest` node.

Toolchain install (Windows, from scratch) is in [Miner/docs/setup.md](Miner/docs/setup.md): MSVC
Build Tools (linker), rustup, Bitcoin Core. The GPU crate additionally needs the **CUDA Toolkit
12.x** (`nvcc` + `CUDA_PATH` on PATH) — `cudarc` detects the version at build time via the
`cuda-version-from-build-system` feature.

## Architecture

Both miners share the same pipeline and nearly identical helper modules (`rpc.rs`, `template.rs`,
`coinbase.rs`, `merkle.rs`, `config.rs`). The GPU crate's copies are forks of the CPU ones — when
fixing a bug in shared logic, check whether the twin needs the same fix.

The core loop (`mining.rs` in each crate):

1. `rpc.rs` calls `getblocktemplate`; `template.rs` parses it into a `BlockTemplate`.
2. `MiningJob::from_template` converts to header-ready form — note the **byte-order conversions**:
   `previous_blockhash` and txids are reversed to internal order; `bits` is reversed because the
   template gives big-endian but the header field is little-endian; `target` is kept big-endian
   (display order) for comparison.
3. Per extranonce: `coinbase.rs` builds the coinbase tx (BIP34 height + tag + extranonce in
   scriptSig, witness commitment if present), `merkle.rs` computes the merkle root over
   `[coinbase_txid, ...other_txids]`, and the 80-byte header skeleton is assembled (bytes 76..80 =
   nonce, left for the search to vary).
4. Search the 2³² nonce space. On exhaustion, increment extranonce (changes the coinbase → new
   merkle root → fresh search space).
5. On a hit, **re-verify the hash on the host** before trusting it, then `assemble_block_hex` and
   `submitblock`.
6. The template is refreshed every `refresh_seconds / 6`; a new height or newer `curtime` swaps the
   job and restarts the search (mining on a stale template = wasted work).

`is_below_target` compares the little-endian internal hash against the big-endian target by walking
`hash[31-i]` vs `target[i]` — get this wrong and valid blocks are missed or invalid ones submitted.

**CPU search-space partitioning:** N worker threads, each owns `extranonce = (thread_id << 32) |
local_counter`, so threads never collide. Template swaps are detected via an `Arc<MiningJob>`
pointer comparison (`ArcSwap`, a tiny inline shim, not the crate).

**GPU pipeline** ([Miner/gpu/](Miner/gpu/)):

- `sha256_host.rs` computes the SHA-256 **midstate** after the header's first 64 bytes. Only the
  last 16 bytes change with the nonce, so this is computed once per (job, extranonce) on the host.
- `kernels/sha256d.cu` — each GPU thread resumes from the midstate, finishes the first SHA-256,
  does the second, and compares to the target. Millions of nonces per launch.
- `gpu.rs` — **all `cudarc`-specific API usage is isolated here on purpose.** Compiles the `.cu` to
  PTX at runtime via NVRTC, keeps reusable device buffers, dispatches batches, reads back hits. If
  a `cudarc` version bump breaks the build, this is the only file to touch (`CudaDevice::new`,
  `load_ptx`, `htod_sync_copy_into`, `dtoh_sync_copy`, `get_func`, `LaunchConfig`, `func.launch`).
  The target GPU arch is the `ARCH` const (`sm_75` for the GTX 1650 / Turing) — change it for other
  cards.

## Conventions and gotchas

- `payout_script_hex` in config is the **scriptPubKey hex**, not a bech32 address. Get it via
  `bitcoin-cli getaddressinfo $(bitcoin-cli getnewaddress) | jq -r .scriptPubKey`.
- Validate correctness on **regtest** (finds a block in milliseconds, `submitblock OK`) before
  pointing at testnet4/mainnet. Node setup per network: [Miner/docs/nets/](Miner/docs/nets/).
- Mainnet requires a fully synced node; mining on a desynced node mines a dead fork.
- Benchmark results are recorded in [Miner/docs/banchmarks/](Miner/docs/banchmarks/) (note the
  misspelled dir) and [Miner/docs/hashrates.md](Miner/docs/hashrates.md). The GPU and CPU miners
  print the same `MH/s | hashes=` stats line every 5s for apples-to-apples comparison.
- Release profile is aggressive (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`) — clean
  release builds are slow; iterate with debug builds.
