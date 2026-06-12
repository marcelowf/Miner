# Bitcoin GPU Miner (CUDA) — Loteria de Bloco

Versão **GPU** do `btc-lottery-miner`. Mesma ideia do projeto de CPU (conecta a um nó Bitcoin Core
via RPC, monta o header e procura um nonce com `SHA256(SHA256(header)) < target`), mas o hot loop
roda na **GPU NVIDIA** via CUDA. Feito para a **GTX 1650** desta máquina (PC2).

> Continua sendo uma **loteria**. A GPU faz a busca ~100x mais rápida que a CPU desta máquina, mas
> contra a rede (~700 EH/s) a chance de achar um bloco na mainnet segue desprezível. É o teto do que
> esta placa consegue — um bilhete de loteria comprado mais rápido.

O miner de CPU original fica em `../cpu/` e **não é alterado** por este projeto.

---

## Como funciona (diferença para a versão CPU)

O header tem 80 bytes = 2 blocos SHA-256. Os primeiros 64 bytes não mudam enquanto só o nonce varia.
Então:

1. **No host (CPU/Rust):** por template + extranonce, monta a coinbase, calcula o merkle root, monta
   o header e o **midstate** (estado SHA-256 após os primeiros 64 bytes) — ver `src/sha256_host.rs`.
2. **Na GPU (kernel CUDA):** cada thread pega um nonce, termina o 1º SHA-256 a partir do midstate,
   faz o 2º SHA-256 e compara com o target — ver `kernels/sha256d.cu`. Milhões de nonces em paralelo.
3. **De volta no host:** se a GPU achou um nonce válido, o host **reconfere** o hash, monta o bloco e
   chama `submitblock`.

---

## Pré-requisitos

- **Rust toolchain** (`rustup` + `cargo`) — ver `../docs/setup.md`, passos 1–4.
- **Driver NVIDIA** atualizado (a GTX 1650 já funciona nesta máquina).
- **CUDA Toolkit 12.x** — fornece o `nvcc`/NVRTC e os headers que o crate `cudarc` usa para compilar
  e ligar. A GTX 1650 é Turing (compute capability **sm_75**), suportada por todo o CUDA 11/12.
  - Instale de https://developer.nvidia.com/cuda-downloads (Windows x86_64). Ocupa ~3 GB.
  - Confirme depois: `nvcc --version` deve responder, e a variável `CUDA_PATH` deve existir.
- **Bitcoin Core** — só é necessário **rodar** o miner. Para validar correção, **regtest basta**
  (sem sincronizar nada). Para minerar de verdade, um nó mainnet/testnet sincronizado + carteira.

---

## Build

```powershell
cd c:\Develop\Miner\gpu
cargo build --release
```

Binário: `target\release\btc-lottery-miner-gpu.exe`.

> ⚠️ **Nota honesta:** este código foi escrito numa máquina **sem** o toolchain Rust/CUDA instalado,
> então a primeira compilação aqui é o primeiro teste real da integração com o `cudarc`. O kernel
> SHA-256 e o midstate foram validados por raciocínio + testes (`cargo test`), mas a *versão/feature*
> do `cudarc` pode pedir um ajuste fino — ver "Solução de problemas de build" abaixo. Toda a parte
> específica de CUDA está isolada em `src/gpu.rs`.

Rode os testes do SHA-256 (validam o midstate contra o crate `sha2`):

```powershell
cargo test
```

---

## Confirmar que a GPU é vista (não precisa de nó)

```powershell
.\target\release\btc-lottery-miner-gpu.exe --list-devices
```

Deve listar algo como `[0] NVIDIA GeForce GTX 1650`.

---

## Configuração

Copie `config.example.toml` para `config.toml` e edite (mesma estrutura da versão CPU + seção
`[gpu]`):

```toml
[rpc]
url = "http://127.0.0.1:18443"   # regtest=18443, testnet4=48332, mainnet=8332
user = "miner"
password = "trocar_isto"

[mining]
payout_script_hex = "0014..."    # scriptPubKey da SUA carteira (não o endereço bech32)
refresh_seconds = 30
coinbase_tag = "/btc-lottery-miner-gpu/"

[gpu]
device_index = 0
threads_per_block = 256
blocks = 4096                    # "intensity" — sobe até saturar a GPU
nonces_per_thread = 64
```

---

## Rodar

**Minerar (envia bloco se achar):**
```powershell
.\target\release\btc-lottery-miner-gpu.exe --config config.toml
```

**Benchmark (mede o hashrate puro, NÃO envia bloco):**
```powershell
.\target\release\btc-lottery-miner-gpu.exe --config config.toml --benchmark
```
No modo `--benchmark` o target é ignorado (nenhum acerto é reportado), então mede a velocidade real
da GPU independente da dificuldade da rede — funciona até contra um nó **regtest** (que é trivial de
subir, sem sincronizar). É a forma recomendada de medir a GTX 1650 sem um nó mainnet sincronizado.

Saída esperada (mesmo formato da versão CPU, pra comparar direto no `pc2.md`):

```
[...] btc-lottery-miner-gpu v0.1.0
[...] Connected to node — chain=regtest, height=0
[...] GPU: [0] NVIDIA GeForce GTX 1650
[...] GPU grid: 4096 blocks × 256 threads × 64 nonces = 67108864 nonces/launch
[...] Initial template: height=1, txs=0, value=5000000000 sat
[...]  412.93 MH/s | hashes=2013265920
```

Pare com `Ctrl+C`.

---

## Validar correção (regtest)

O teste decisivo de que todo o caminho GPU está correto (byte-order, coinbase, header) é o nó
**aceitar** um bloco que a GPU minerou:

1. Suba um `bitcoind -regtest` com RPC (ver `../docs/regtest.md`).
2. Gere uma carteira/endereço no regtest e ponha o `scriptPubKey` em `config.toml`.
3. Rode **sem** `--benchmark`. Como o target do regtest é fácil, deve achar e enviar um bloco em
   menos de um segundo, com `submitblock OK`. Confirme com `bitcoin-cli -regtest getblockcount`.

Só depois disso vale apontar para mainnet/testnet.

---

## Solução de problemas de build

O ponto de integração mais provável de exigir ajuste é o `cudarc` (no `Cargo.toml`):

- **Erro de versão do CUDA / `cuda-version-from-build-system` não detecta**: garanta que `nvcc` está
  no PATH e `CUDA_PATH` aponta para o toolkit. Alternativa: troque a feature por uma fixa que case
  com seu toolkit, ex. `"cuda-12060"`, ou use `"dynamic-loading"` (carrega as libs CUDA em runtime).
- **Linker não acha `nvrtc`/`cuda`**: confirme que o CUDA Toolkit (não só o driver) está instalado.
- **API do `cudarc` mudou** (se você pegou uma versão diferente da 0.12): os pontos a conferir estão
  todos em `src/gpu.rs` — `CudaDevice::new`, `load_ptx`, `htod_sync_copy_into`, `dtoh_sync_copy`,
  `get_func`, `LaunchConfig` e `func.launch`.
- **GPU diferente da GTX 1650**: ajuste a constante `ARCH` em `src/gpu.rs` (ex. `sm_86` para Ampère).

---

## Estrutura

```
gpu/
  kernels/sha256d.cu   # kernel CUDA (SHA-256d + comparação com target)
  src/
    main.rs            # CLI (--config, --log, --list-devices, --benchmark)
    config.rs          # config TOML ([rpc] [mining] [gpu])
    rpc.rs             # cliente JSON-RPC do Bitcoin Core   (cópia do Miner)
    template.rs        # parse do getblocktemplate           (cópia do Miner)
    coinbase.rs        # monta a coinbase                    (cópia do Miner)
    merkle.rs          # merkle root + sha256d no host       (cópia do Miner)
    sha256_host.rs     # midstate dos primeiros 64 bytes
    gpu.rs             # cudarc: device, NVRTC, dispatch/readback
    mining.rs          # loop: host monta header+midstate, GPU varre nonces
```

---

## Licença

MIT. É um bilhete de loteria, faça o que quiser com ele.
