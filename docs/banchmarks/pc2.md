# PC2 — Benchmark

## Especificações

| Componente | Detalhe |
|---|---|
| CPU | Intel(R) Core(TM) i5-9400F @ 2.90GHz (6 cores / 6 threads) |
| GPU | NVIDIA GeForce GTX 1650 (4 GB dedicada) |
| RAM | ~15.93 GB |
| SO | Windows 10 Pro, 64 bits (10.0.19045) |

> **Plano desta máquina:** minerar pela **GPU** (GTX 1650) com o novo projeto `../../gpu/`
> (CUDA). Estimativa ~300-600 MH/s (~100x a CPU desta máquina). O miner de CPU em `Miner/` continua
> disponível para comparação.

## Config do miner

_pendente — `gpu/config.toml` ainda não existe (só `config.example.toml`)_

## Durante o benchmark (Gerenciador de Tarefas)

_pendente_

## Hashrate observado

_pendente — medir com `btc-lottery-miner-gpu.exe --benchmark`_

## TODO — pendências antes de rodar (GPU / CUDA)

Toolchain e GPU:
- [ ] Instalar toolchain Rust (rustup/cargo) — ver `docs/setup.md`
- [ ] Instalar **CUDA Toolkit 12.x** (~3 GB) — traz nvcc/NVRTC; confirmar `nvcc --version` e `CUDA_PATH`
- [ ] `cd gpu && cargo build --release`
- [ ] `cargo test` (valida o SHA-256/midstate contra o crate `sha2`)
- [ ] `btc-lottery-miner-gpu.exe --list-devices` → deve listar a GTX 1650

Medir hashrate (NÃO precisa de nó sincronizado):
- [ ] Subir um `bitcoind -regtest` (rápido, sem sync) e criar `gpu/config.toml`
- [ ] Rodar `--benchmark` por alguns minutos, coletar as linhas `MH/s`
- [ ] Anotar uso de GPU/CPU/Memória no Gerenciador de Tarefas (aba Desempenho → GPU) e preencher acima

Validar correção (regtest) e minerar de verdade:
- [ ] Gerar carteira/endereço no regtest e pôr o `payout_script_hex` no config
- [ ] Rodar **sem** `--benchmark` em regtest → deve achar e enviar bloco em <1s (`submitblock OK`)
- [ ] Para minerar pra valer: parar/conectar o nó mainnet/testnet sincronizado + trazer a carteira

Depois de medir:
- [ ] Adicionar a linha da GPU (GTX 1650) em `docs/hashrates.md` com o número real medido
