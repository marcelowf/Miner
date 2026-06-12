//! CUDA glue (via the `cudarc` crate). All GPU-specific API usage is isolated here so that if a
//! `cudarc` version bump changes the surface, only this file needs touching.
//!
//! Responsibilities:
//!   * pick the CUDA device,
//!   * compile `kernels/sha256d.cu` to PTX at runtime with NVRTC,
//!   * keep reusable device buffers (midstate, target, output),
//!   * dispatch one batch of nonces and read back any hits.

use anyhow::{anyhow, Context, Result};
use cudarc::driver::{CudaDevice, CudaSlice, LaunchAsync, LaunchConfig};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use std::sync::Arc;

/// Compute capability of the target GPU. GTX 1650 (Turing) = sm_75.
/// Change this if you build for a different card.
const ARCH: &str = "sm_75";

/// The CUDA C kernel source, compiled into the binary.
const KERNEL_SRC: &str = include_str!("../kernels/sha256d.cu");

/// Max hits we record per launch. We only ever need one valid block; on regtest (easy target)
/// many nonces qualify, so we cap the buffer and just use the first verified hit.
const OUT_CAPACITY: u32 = 256;

/// List all visible CUDA devices (used by `--list-devices`). Does not need a node.
pub fn list_devices() -> Result<()> {
    let mut found = 0usize;
    loop {
        match CudaDevice::new(found) {
            Ok(dev) => {
                let name = dev.name().unwrap_or_else(|_| "<sem nome>".to_string());
                println!("[{found}] {name}");
                found += 1;
            }
            Err(_) => break,
        }
    }
    if found == 0 {
        println!("Nenhum dispositivo CUDA encontrado (driver NVIDIA instalado?).");
    }
    Ok(())
}

pub struct Gpu {
    dev: Arc<CudaDevice>,
    midstate_buf: CudaSlice<u32>,
    target_buf: CudaSlice<u32>,
    out_count: CudaSlice<u32>,
    out_nonces: CudaSlice<u32>,
    blocks: u32,
    threads_per_block: u32,
    nonces_per_thread: u32,
}

impl Gpu {
    pub fn new(
        device_index: usize,
        blocks: u32,
        threads_per_block: u32,
        nonces_per_thread: u32,
    ) -> Result<Self> {
        let dev = CudaDevice::new(device_index)
            .with_context(|| format!("falha ao abrir dispositivo CUDA {device_index}"))?;
        log::info!(
            "GPU: [{device_index}] {}",
            dev.name().unwrap_or_else(|_| "<sem nome>".to_string())
        );

        // Compile the kernel to PTX for the target architecture.
        let opts = CompileOptions {
            arch: Some(ARCH),
            ..Default::default()
        };
        let ptx = compile_ptx_with_opts(KERNEL_SRC, opts)
            .map_err(|e| anyhow!("falha ao compilar kernel CUDA (NVRTC): {e}"))?;
        dev.load_ptx(ptx, "sha256d", &["mine"])
            .map_err(|e| anyhow!("falha ao carregar PTX no device: {e}"))?;

        let midstate_buf = dev.alloc_zeros::<u32>(8)?;
        let target_buf = dev.alloc_zeros::<u32>(8)?;
        let out_count = dev.alloc_zeros::<u32>(1)?;
        let out_nonces = dev.alloc_zeros::<u32>(OUT_CAPACITY as usize)?;

        Ok(Gpu {
            dev,
            midstate_buf,
            target_buf,
            out_count,
            out_nonces,
            blocks,
            threads_per_block,
            nonces_per_thread,
        })
    }

    /// Nonces covered by one `run_batch` call.
    pub fn batch_span(&self) -> u64 {
        self.blocks as u64 * self.threads_per_block as u64 * self.nonces_per_thread as u64
    }

    /// Hash `base_nonce .. base_nonce + batch_span()` and return any nonces whose double-SHA256
    /// header hash is below `target`. `midstate` is the SHA-256 state after the first 64 header
    /// bytes; `tail` is header bytes 64..76 packed big-endian (merkle tail, time, bits);
    /// `target` is 8 big-endian words, most significant first.
    pub fn run_batch(
        &mut self,
        midstate: &[u32; 8],
        tail: [u32; 3],
        target: &[u32; 8],
        base_nonce: u32,
    ) -> Result<Vec<u32>> {
        self.dev
            .htod_sync_copy_into(midstate, &mut self.midstate_buf)?;
        self.dev.htod_sync_copy_into(target, &mut self.target_buf)?;
        // Reset the hit counter to zero.
        self.dev
            .htod_sync_copy_into(&[0u32], &mut self.out_count)?;

        let func = self
            .dev
            .get_func("sha256d", "mine")
            .ok_or_else(|| anyhow!("função de kernel 'mine' não encontrada"))?;

        let cfg = LaunchConfig {
            grid_dim: (self.blocks, 1, 1),
            block_dim: (self.threads_per_block, 1, 1),
            shared_mem_bytes: 0,
        };

        // SAFETY: argument types/order match the `mine` kernel signature in sha256d.cu.
        unsafe {
            func.launch(
                cfg,
                (
                    &self.midstate_buf,
                    tail[0],
                    tail[1],
                    tail[2],
                    &self.target_buf,
                    base_nonce,
                    self.nonces_per_thread,
                    &mut self.out_count,
                    &mut self.out_nonces,
                    OUT_CAPACITY,
                ),
            )?;
        }

        let count = self.dev.dtoh_sync_copy(&self.out_count)?[0].min(OUT_CAPACITY);
        if count == 0 {
            return Ok(Vec::new());
        }
        let all = self.dev.dtoh_sync_copy(&self.out_nonces)?;
        Ok(all[..count as usize].to_vec())
    }
}
