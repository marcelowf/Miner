# Mainnet — Mineração Bitcoin real

Guia para rodar o `btc-lottery-miner` na mainnet — rede Bitcoin principal com BTC real.

**Pré-requisito:** ambiente instalado conforme [setup.md](setup.md), regtest validado conforme [regtest.md](regtest.md) e testnet4 validado conforme [testnet4.md](testnet4.md)

> **Aviso:** na mainnet o BTC é real. A recompensa de bloco vai para o endereço configurado no `config.toml`. Use um endereço de uma carteira segura (Sparrow, hardware wallet) — nunca da wallet interna do nó.

---

## Preparação do armazenamento ✅

A blockchain mainnet ocupa ~870 GB e cresce ~1 GB/semana. É necessário um disco dedicado com espaço suficiente.

### Discos disponíveis nesta máquina

```powershell
Get-PhysicalDisk | Select-Object FriendlyName, MediaType, @{N='RPM';E={$_.SpindleSpeed}}, @{N='Tamanho(GB)';E={[math]::Round($_.Size/1GB,0)}}
```

Resultado:

| Disco | Tipo | Tamanho | Uso |
|---|---|---|---|
| IM2P33F3A NVMe ADATA 512GB | SSD NVMe | 477 GB | Sistema (C:) |
| ST1000NM0018-2F2130 (Seagate) | HDD 7200 RPM | 932 GB | D: — disponível |
| WD Elements 2621 | HDD USB externo | 1863 GB | F: — **blockchain atual** |

### Jornada de armazenamento

#### Tentativa 1: Seagate D: (04/06 a 08/06/2026)

O Seagate (932 GB) foi configurado como D: e usado para o sync inicial:

```powershell
# Requer PowerShell como Administrador
Set-Partition -DiskNumber 0 -PartitionNumber 3 -NewDriveLetter D
Format-Volume -DriveLetter D -FileSystem NTFS -NewFileSystemLabel "Bitcoin" -Confirm:$false
New-Item -ItemType Directory -Force "D:\Bitcoin" | Out-Null
```

O sync progrediu de 04/06 a 08/06 atingindo apenas ~10.8% (~424k blocos). O gargalo foi o disco mecânico:

```
CPU: 3%  |  Memória: 89%  |  Disco: 46%  |  Rede: 0%
```

Velocidade real: ~600 blocos/hora. Estimativa para completar: ~35 dias.

**Conclusão:** HDD mecânico é inviável para IBD. O gargalo não é banda de internet nem CPU — é I/O de disco.

#### Migração para SSD (plano em andamento)

Foi adquirido um SSD NVMe M.2 Gen4 x4 de 1 TB (Hiksemi, até 7100 MB/s). Como o slot M.2 da placa-mãe já está ocupado pelo ADATA, será usado via enclosure USB externo.

**Enclosure recomendado:** UGREEN 90264 (chip Realtek RTL9210B)
- Velocidade real via USB 3.2 Gen2: ~950 MB/s (~6x mais rápido que o HDD)
- Tempo estimado para IBD completo no SSD: ~27 horas (medido em outra máquina)
- Evitar chips JMicron JMS583 — risco de superaquecimento e corrupção de dados em uso contínuo

> Enclosure aguardando entrega. Quando chegar: sincronizar no SSD, depois copiar para D: se necessário (~1h40min a ~120 MB/s).

#### Solução atual: WD Elements 2TB em F: (09/06/2026) ✅

Um HD externo WD Elements 2TB (USB) já continha 868 GB de blockchain pré-sincronizada. Conectado como F:, o nó completou o sync em minutos.

```powershell
# Verificar conteúdo do disco conectado
Get-ChildItem F:\ | Select-Object Name, LastWriteTime
Get-ChildItem F:\bitcoin-data | Select-Object Name, LastWriteTime

# Tamanho total
"{0:N1} GB" -f ((Get-ChildItem F:\bitcoin-data -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1GB)
# Resultado: 868,4 GB
```

---

## Diferenças em relação à testnet4

| | Testnet4 | **Mainnet** |
|---|---|---|
| BTC | tBTC (sem valor) | BTC real |
| Blockchain | ~10 GB | ~870 GB |
| Sync (HDD) | ~2h | ~35 dias |
| Sync (SSD) | ~2h | ~27 horas |
| Dificuldade | real, baixa | real, altíssima |
| CPU acha bloco | horas ou nunca | impossível na prática |
| Endereço | `tb1...` | `bc1...` |
| Porta RPC | 48332 | 8332 |
| Flag bitcoind | `-testnet4` | nenhuma (default) |

---

## Hardware necessário

| Recurso | Mínimo | Esta máquina |
|---|---|---|
| Disco para blockchain | ~950 GB | WD Elements 2TB (F:) ✅ |
| RAM durante sync | ~2 GB | 4 GB (usar `-dbcache=256`) ✅ |
| RAM idle (pós-sync) | ~500 MB | 4 GB ✅ |
| Conexão | estável | WiFi 1.2 Gbps ✅ |

> Com 4 GB de RAM, feche outros programas durante o sync. O `-dbcache=256` limita o uso de RAM do bitcoind.

---

## Fase 1 — Endereço de payout ✅

Na mainnet o endereço deve vir de uma carteira com controle total das chaves privadas. **Nunca use a wallet interna do bitcoind para receber recompensas reais.**

### Obter o scriptPubKey do Sparrow

1. Abra o **Sparrow Wallet**
2. Selecione sua carteira → aba **Receive**
3. Copie o endereço `bc1q...` (Native SegWit / P2WPKH)
4. Obtenha o scriptPubKey via bitcoin-cli:

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -datadir=F:\bitcoin-data validateaddress "SEU_ENDERECO_BC1Q" | ConvertFrom-Json | Select-Object -ExpandProperty scriptPubKey
```

**Resultado real obtido:**
- Endereço: `bc1q...`
- scriptPubKey: `0014...` (P2WPKH)
- Derivação: `m/84'/0'/0'/0/0` (BIP84 Native SegWit)

O scriptPubKey tem formato `0014<40 chars hex>` (P2WPKH) ou `0020<64 chars hex>` (P2WSH).

---

## Fase 2 — Configurar bitcoin.conf ✅

O conf do nó fica dentro do próprio datadir: `F:\bitcoin-data\bitcoin.conf`

Mainnet é o default — sem flag de rede.

### Problema encontrado no conf original

O bitcoin.conf que veio com o WD Elements tinha configurações incompatíveis com Windows:

```
# PROBLEMÁTICO — não usar:
daemon=1      ← não funciona no Windows
dbcache=4096  ← 4 GB de cache com 4 GB de RAM total → trava o sistema
```

### Correção aplicada

> **Atenção:** usar `Set-Content` com array de strings. O here-string `@'...'@` do PowerShell pode escrever o delimitador literalmente no arquivo causando parse error no bitcoind.

```powershell
Set-Content "F:\bitcoin-data\bitcoin.conf" -Encoding ascii -Value @(
    "server=1",
    "txindex=1",
    "dbcache=256",
    "rpcuser=miner",
    "rpcpassword=lottery",
    "rpcbind=127.0.0.1",
    "rpcallowip=127.0.0.1"
)
Get-Content "F:\bitcoin-data\bitcoin.conf"
```

**Conteúdo aplicado:**
```
server=1
txindex=1
dbcache=256
rpcuser=miner
rpcpassword=lottery
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
```

> `txindex=1` foi mantido pois o sync completo foi feito com ele — remover forçaria reindexação de toda a blockchain.

---

## Fase 3 — Sincronizar ✅ concluído

A mainnet tem ~952k blocos e ~870 GB de dados.

### Iniciar o nó

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
Start-Process -FilePath bitcoind -ArgumentList "-datadir=F:\bitcoin-data -dbcache=256" -WindowStyle Minimized
```

> **Windows não suporta `-daemon`.** Use sempre `Start-Process` para rodar em background.

### Monitorar progresso

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -datadir=F:\bitcoin-data getblockchaininfo | ConvertFrom-Json |
  Select-Object chain, blocks, headers, verificationprogress
```

Quando `verificationprogress` ≥ `0.9999`, está sincronizado.

### O sync tem duas fases

1. **Headers** — rápido (~minutos): baixa os cabeçalhos de todos os ~952k blocos
2. **IBD (Initial Block Download)** — lento (~horas/dias): baixa e valida cada bloco completo

### Progresso registrado (sync inicial no Seagate D:, 2026-06-04/08)

| Hora | Blocos | Headers | Progresso |
|---|---|---|---|
| 20:45 | 0 | 0 | ~0% — iniciando |
| 20:50 | 0 | 952.408 | ~0% — headers concluídos |
| 20:55 | 4.771 | 952.408 | ~0.00035% |
| 21:00 | 34.783 | 952.408 | ~0.0026% |
| 21:05 | 69.320 | 952.410 | ~0.0065% |
| 21:10 | 103.063 | 952.410 | ~0.017% |
| 21:15 | 136.280 | 952.410 | ~0.076% |
| 21:20 | 167.528 | 952.410 | ~0.18% |
| 21:25 | 196.077 | 952.410 | ~0.48% |
| 21:30 | 221.274 | 952.410 | ~0.94% |
| *(overnight) 04/06/2026 => 05/06/2026* | — | — | — |
| 08:05 | 379.692 | 952.466 | ~6.5% |
| 20:15 | 399.189 | 952.542 | ~8.2% |
| 23:15 | 402.186 | 952.558 | ~8.5% |
| *(overnight) 05/06/2026 => 06/06/2026* | — | — | — |
| 07:35 | 409.004 | 952.596 | ~9.2% |
| 21:40 | 417.539 | 952.666 | ~10.1% |
| *(nó parado — tentativa de instalação SSD)* | — | — | — |
| 22:10 | 417.622 | 952.668 | ~10.1% |
| *(overnight) 06/06/2026 => 07/06/2026* | — | — | — |
| 08:50 | 424.301 | 952.716 | ~10.8% |
| *(HD D: limpo — migração para WD Elements com blockchain existente)* | — | — | — |
| **09/06/2026** | **952.921** | **952.921** | **100% ✅** |

> Blocos antigos (pré-2012) são pequenos e processam rápido. A velocidade cai drasticamente nos blocos pós-2017 (transações maiores, SegWit, Taproot).

### Estimativa de tempo

O gargalo real do IBD **não é a banda de internet** — é o disco. A conexão de rede fica ociosa enquanto a CPU valida e o disco grava.

| Storage | Tempo real medido | Observação |
|---|---|---|
| SSD (NVMe ou SATA) | **~27 horas** ✅ medido | Gargalo: validação CPU |
| HDD 7200 RPM | ~35+ dias (estimado) | Gargalo: I/O disco |
| HDD 5400 RPM externo USB | ainda mais lento | — |

> **Use SSD.** A diferença é de dias vs semanas.

### Verificar uso do disco durante sync

```powershell
Get-PSDrive F | Select-Object Name, @{N='Usado(GB)';E={[math]::Round($_.Used/1GB,1)}}, @{N='Livre(GB)';E={[math]::Round($_.Free/1GB,1)}}
```

---

## Fase 3.5 — Conectar Sparrow ao nó local ✅

Com o nó sincronizado, o Sparrow pode ser conectado diretamente a ele em vez de servidores externos — mais privado e sem dependência de terceiros.

### Configurar no Sparrow

1. **File → Preferences → Server**
2. Selecione **Bitcoin Core**
3. Preencha:
   - **URL:** `127.0.0.1`
   - **Port:** `8332`
   - **User:** `miner`
   - **Password:** `lottery`
4. Clique em **Test Connection**

**Resultado real obtido:**
```
Connected to Cormorant 2.5.1 on protocol version 1.4
Batched RPC enabled.
Server Banner: Cormorant 2.5.1
/Satoshi:31.0.0/
```

O nó expõe o protocolo Electrum via Cormorant (servidor Electrum embutido). O Sparrow usa essa interface para consultar saldos e transações diretamente do seu próprio nó.

---

## Fase 4 — Carteira e config.toml ✅

### config.toml aplicado

```powershell
Set-Content "C:\Develop\Miner\config.toml" -Encoding ascii -Value @(
    "[rpc]",
    'url = "http://127.0.0.1:8332"',
    'user = "miner"',
    'password = "lottery"',
    "",
    "[mining]",
    'payout_script_hex = "0014..."',
    "threads = 0",
    "refresh_seconds = 30",
    'coinbase_tag = "/btc-lottery-miner/"'
)
Get-Content "C:\Develop\Miner\config.toml"
```

**Conteúdo:**
```toml
[rpc]
url = "http://127.0.0.1:8332"
user = "miner"
password = "lottery"

[mining]
payout_script_hex = "0014..."
threads = 0
refresh_seconds = 30
coinbase_tag = "/btc-lottery-miner/"
```

### Portas RPC por rede

| Rede | Porta RPC | Flag bitcoind |
|---|---|---|
| Regtest | 18443 | `-regtest` |
| Testnet4 | 48332 | `-testnet4` |
| **Mainnet** | **8332** | nenhuma |

---

## Fase 5 — Rodar o miner ✅ ativo

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
Set-Location C:\Develop\Miner
cargo build --release
.\target\release\btc-lottery-miner.exe --config config.toml
```

### Problema encontrado: config.toml com configurações da testnet4

O `config.toml` ainda apontava para a porta 48332 (testnet4) e tinha o scriptPubKey antigo. Corrigido com:

```powershell
Set-Content "C:\Develop\Miner\config.toml" -Encoding ascii -Value @(
    "[rpc]",
    'url = "http://127.0.0.1:8332"',
    'user = "miner"',
    'password = "lottery"',
    "",
    "[mining]",
    'payout_script_hex = "0014..."',
    "threads = 0",
    "refresh_seconds = 30",
    'coinbase_tag = "/btc-lottery-miner/"'
)
```

### Output real obtido (09/06/2026)

```
[2026-06-09T03:32:14Z INFO  btc_lottery_miner] btc-lottery-miner v0.1.0
[2026-06-09T03:32:14Z INFO  btc_lottery_miner::mining] Connected to node — chain=main, height=952922
[2026-06-09T03:32:14Z INFO  btc_lottery_miner::mining] Initial template: height=952923, txs=4703, value=314938700 sat
[2026-06-09T03:32:14Z INFO  btc_lottery_miner::mining] Mining with 16 threads
[2026-06-09T03:32:19Z INFO  btc_lottery_miner::mining]    9.75 MH/s | hashes=49479680
[2026-06-09T03:32:25Z INFO  btc_lottery_miner::mining]   10.16 MH/s | hashes=101187584
```

**Diferenças visíveis em relação à testnet4:**

| Campo | Testnet4 | Mainnet |
|---|---|---|
| `chain` | `testnet4` | `main` |
| `txs` | 4 | 4703 (mempool real) |
| `value` | ~5 BTC (recompensa) | ~3.15 BTC (só fees — subsídio caiu após halving) |
| Hashrate | ~10 MH/s | ~10 MH/s (igual) |
| Bloco encontrado | horas ou nunca | praticamente impossível |

### O que esperar

O miner funciona exatamente igual ao testnet4 — mesma saída, mesmo hashrate. A diferença está na dificuldade:

- Seu hashrate: ~10 MH/s
- Rede mainnet: ~800 EH/s (~800.000.000.000 MH/s)
- Sua fração: ~0.000000000001% do hashrate total
- Probabilidade de achar um bloco por dia: praticamente zero

**Isso não é um problema** — o projeto é educacional. O valor está em rodar o fluxo completo com BTC real: conectar ao nó, receber templates com transações reais da mainnet, submeter blocos válidos.

---

## Como saber se (milagrosamente) encontrou um bloco

### 1. Output do terminal
```
[INFO] ★ BLOCK FOUND ★ thread=X nonce=Y extranonce=0xZ
[INFO] hash = <hash>
[INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

### 2. Block explorer
Busque o hash em `mempool.space` — o coinbase terá `/btc-lottery-miner/` como tag.

### 3. Verificar saldo no Sparrow
Abra o Sparrow e veja o endereço de recebimento — aparecerá uma transação com `immature` (aguarda 100 confirmações ~17h).

---

## Encerrar o nó

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -datadir=F:\bitcoin-data stop
```

---

## Retomar em uma nova sessão

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
Start-Process -FilePath bitcoind -ArgumentList "-datadir=F:\bitcoin-data -dbcache=256" -WindowStyle Minimized
Start-Sleep -Seconds 10
bitcoin-cli -datadir=F:\bitcoin-data getblockchaininfo | ConvertFrom-Json |
  Select-Object chain, blocks, verificationprogress
```

O nó retoma e sincroniza os blocos novos automaticamente (apenas os que faltam desde o último shutdown).

---

## Segurança

- `config.toml` está no `.gitignore` — nunca sobe para o repositório
- O scriptPubKey é público (derivado do endereço) — não expõe chaves privadas
- As chaves privadas ficam exclusivamente no Sparrow — nunca no nó
- Se o WD Elements for perdido ou formatado, apenas a blockchain é perdida (re-sincronizável em ~27h com SSD) — os fundos ficam seguros no Sparrow
