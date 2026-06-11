# PC1 — Benchmark

## Especificações

| Componente | Detalhe |
|---|---|
| CPU | Intel(R) Core(TM) i7-10700 @ 2.90GHz (8 cores / 16 threads) |
| GPU | Intel(R) UHD Graphics 630 (integrada, 1 GB) — sem GPU dedicada |
| RAM | ~3.75 GB |
| SO | Windows 11 Home Single Language, 64 bits (10.0.26200) |

## Config do miner

```toml
threads = 0  # auto
```

## Durante o benchmark (Gerenciador de Tarefas)

| Recurso | Uso |
|---|---|
| CPU | 100% @ 3.74 GHz |
| Memória | 3,4/3,8 GB (89%) |
| Disco: (SSD, RAID) | 18% |
| GPU 0 (Intel UHD) | 2% |

## Hashrate observado

~9 MH/s (média)

```
[2026-06-09T11:55:00Z INFO  btc_lottery_miner::mining]    9.26 MH/s | hashes=268390105088
[2026-06-09T14:35:00Z INFO  btc_lottery_miner::mining]    8.85 MH/s | hashes=353692680192
[2026-06-09T14:45:00Z INFO  btc_lottery_miner::mining]    8.39 MH/s | hashes=358932021248
[2026-06-09T19:10:00Z INFO  btc_lottery_miner::mining]    8.72 MH/s | hashes=500212039680
[2026-06-09T20:45:00Z INFO  btc_lottery_miner::mining]    9.13 MH/s | hashes=550990249984
[2026-06-09T23:35:00Z INFO  btc_lottery_miner::mining]    9.07 MH/s | hashes=641952841728
[2026-06-10T11:55:00Z INFO  btc_lottery_miner::mining]    9.11 MH/s | hashes=1037389987840
[2026-06-10T19:05:00Z INFO  btc_lottery_miner::mining]    9.11 MH/s | hashes=1266811535360
[2026-06-11T01:45:00Z INFO  btc_lottery_miner::mining]    9.03 MH/s | hashes=1480832188416
[2026-06-11T14:50:00Z INFO  btc_lottery_miner::mining]    9.16 MH/s | hashes=1900459786240
```
