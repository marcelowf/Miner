# Testnet4 — Mineração em rede pública de testes

Guia para rodar o `btc-lottery-miner` na testnet4 — rede Bitcoin de testes P2P real.

**Pré-requisito:** ambiente instalado conforme [setup.md](setup.md) e regtest validado conforme [regtest.md](regtest.md)

---

## Por que testnet4

| | Regtest | **Testnet4** | Mainnet |
|---|---|---|---|
| Rede | local, isolado | P2P real, pública | P2P real, pública |
| Blockchain | você cria do zero | ~137k blocos reais | ~900k blocos |
| `submitblock` | só seu nó vê | vai para nós reais | vai para nós reais |
| Block explorer | não | `mempool.space/testnet4` | `mempool.space` |
| Dificuldade | mínima (segundos) | real (CPU: horas ou nunca) | real (CPU: impossível) |
| Moeda | BTC falso | tBTC (sem valor) | BTC real |
| Prepara para mainnet | parcialmente | sim — fluxo idêntico | — |

Testnet4 foi criada em 2024, substituindo a testnet3. Bitcoin Core 31.0 suporta nativamente.

---

## Considerações de hardware

| Processo | RAM típica |
|---|---|
| Windows | ~1.5–2 GB |
| bitcoind durante sync (IBD) | ~1–1.5 GB (pico) |
| bitcoind idle (pós-sync) | ~300–500 MB |
| miner (16 threads) | ~50 MB |
| **Total pico** | **~3.5–4 GB** |

> **Máquina com 4 GB de RAM:** usar `-dbcache=256` para limitar o cache do bitcoind (padrão: 450 MB). O sync fica mais lento, mas não estoura a RAM.
>
> - Feche o browser e outros programas durante o sync
> - Não rode o miner enquanto sincroniza — espere concluir

**Disco necessário:** ~10 GB. Sem risco em máquinas com 1 TB+.

---

## Fase 1 — Reconfigurar bitcoin.conf ✅

### Parar o nó regtest

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -regtest stop
```

### Atualizar bitcoin.conf

**Localização:** `%APPDATA%\Bitcoin\bitcoin.conf`

```powershell
$conf = @'
testnet4=1
server=1
[testnet4]
rpcuser=miner
rpcpassword=lottery
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
fallbackfee=0.0001
'@
$conf | Out-File "$env:APPDATA\Bitcoin\bitcoin.conf" -Encoding ascii
Get-Content "$env:APPDATA\Bitcoin\bitcoin.conf"
```

Diferença em relação ao regtest: seção `[testnet4]` e flag `testnet4=1`.

---

## Fase 2 — Sincronizar ✅

O nó precisa baixar e validar todos os blocos da testnet4 (~10 GB) antes de poder minerar.

### Iniciar

```powershell
Start-Process -FilePath bitcoind -ArgumentList "-testnet4 -dbcache=256" -WindowStyle Minimized
```

### Monitorar progresso

```powershell
bitcoin-cli -testnet4 getblockchaininfo | ConvertFrom-Json |
  Select-Object chain, blocks, headers, verificationprogress
```

O sync tem duas fases:
1. **Headers** — rápido, baixa os cabeçalhos de todos os blocos (~minutos)
2. **IBD (Initial Block Download)** — baixa e valida cada bloco completo (~horas)

Quando `verificationprogress` ≥ `0.9999`, está sincronizado.

### Progresso registrado

| Checkpoint | Blocos | Headers | Progresso |
|---|---|---|---|
| Início | 0 | 0 | ~0% |
| +poucos min | 882 | 137.786 | ~0.006% |
| +~20 min | 34.995 | 137.786 | ~5% |
| +~50 min | 85.691 | 137.790 | ~70% |
| +~70 min | 101.619 | 137.790 | ~83% |
| +~85 min | 111.714 | 137.790 | ~92% |
| **Concluído** | **137.791** | **137.791** | **100%** |

**Tempo total real:** ~2 horas (máquina com 4 GB RAM, `-dbcache=256`).

---

## Fase 3 — Carteira e config.toml ✅

Rodar **após sync completo**, na **mesma sessão PowerShell** (`$script` precisa estar na memória):

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -testnet4 createwallet "lottery"
$addr = bitcoin-cli -testnet4 -rpcwallet=lottery getnewaddress "" "bech32"
$script = (bitcoin-cli -testnet4 -rpcwallet=lottery getaddressinfo $addr | ConvertFrom-Json).scriptPubKey
"Endereço: $addr"   # começa com tb1 (bech32 testnet4)
"Script:   $script"
```

**Resultado real obtido:**
- Endereço: `tb1qfeweczlvvxk089d67glyw3zkwcux8f0ar9m37l` (prefixo `tb1` = testnet4 bech32)
- scriptPubKey: `00144e5d9c0bec61acf395baf23e474456763863a5fd` (P2WPKH)

Criar `config.toml` — **única diferença em relação ao regtest: porta 48332**:

```powershell
$cfg = @"
[rpc]
url = "http://127.0.0.1:48332"
user = "miner"
password = "lottery"

[mining]
payout_script_hex = "$script"
threads = 0
refresh_seconds = 30
coinbase_tag = "/btc-lottery-miner/"
"@
$cfg | Out-File "C:\Develop\Miner\config.toml" -Encoding ascii
Get-Content "C:\Develop\Miner\config.toml"
```

### Portas RPC por rede

| Rede | Porta RPC |
|---|---|
| Regtest | 18443 |
| **Testnet4** | **48332** |
| Mainnet | 8332 |

---

## Fase 4 — Rodar o miner ✅ concluído

```powershell
Set-Location C:\Develop\Miner
.\target\release\btc-lottery-miner.exe --config config.toml
```

**Output real obtido:**
```
[INFO] btc-lottery-miner v0.1.0
[INFO] Connected to node — chain=testnet4, height=137791
[INFO] Initial template: height=137792, txs=4, value=5000003490 sat
[INFO] Mining with 16 threads
[INFO]    9.85 MH/s | hashes=50331648
[INFO]   10.15 MH/s | hashes=101580800
```

**Diferenças visíveis em relação ao regtest:**

| Campo | Regtest | Testnet4 |
|---|---|---|
| `chain` | `regtest` | `testnet4` |
| `txs` | 0 (blocos vazios) | 4 (transações reais) |
| `value` | 5000000000 sat (só recompensa) | 5000003490 sat (recompensa + fees) |
| Hashrate | ~10 MH/s | ~10 MH/s (igual) |
| Bloco encontrado | segundos | horas ou nunca |

O miner conecta ao nó, busca o template e minera continuamente. **Na testnet4 ele não para após encontrar um bloco** — continua minerando o próximo.

### Status de execução — histórico completo

| Momento | Evento | Detalhe |
|---|---|---|
| 03:15 | Miner iniciado | chain=testnet4, height=137791 |
| 03:15–03:31 | Minerando bloco 137792 | ~9 MH/s estável, 8.7B hashes |
| ~03:31 | Rede produziu bloco 137792 | Outro minerador ganhou |
| ~03:31 | Template atualizado automaticamente | Miner passou a minerar bloco 137793 |
| 03:31–09:xx | Rodando overnight (~6h) | Acompanhou 63 blocos da rede (137792→137855) |
| 09:xx | Verificação de saldo | `immature: 0` — nenhum bloco encontrado |
| 09:xx | Nó encerrado | `bitcoin-cli -testnet4 stop` |
| 09:xx | Miner encerrado | Processo já havia parado ao fechar terminal |

> Quando a rede produz um novo bloco, o miner descarta o trabalho atual e começa no próximo — comportamento correto confirmado ao longo de 6 horas e 63 transições de template.

**Resultado:** nenhum bloco encontrado — esperado com ~9 MH/s em uma rede com centenas de GH/s de hashrate total. A fração de contribuição é ~0.000001% — é literalmente uma loteria.

---

## Como saber se encontrou um bloco

### 1. Output do terminal
```
[INFO] ★ BLOCK FOUND ★ thread=X nonce=Y extranonce=0xZ
[INFO] hash = <hash do bloco>
[INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

### 2. Block explorer
Busque o hash em `mempool.space/testnet4` — o coinbase terá `/btc-lottery-miner/` como tag, provando que foi você.

### 3. Verificar saldo (em outro terminal)
```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
bitcoin-cli -testnet4 -rpcwallet=lottery getbalances
```
Se aparecer `immature: 50.0` significa que achou um bloco (aguarda 100 confirmações para maturar).

> O miner **não para** automaticamente na testnet4 após encontrar um bloco — continua minerando o próximo. Fique de olho no terminal periodicamente.

---

## O que esperar

### Dificuldade real
A dificuldade na testnet4 é ajustada a cada 2016 blocos. Com CPU, achar um bloco pode levar **horas, dias ou nunca** — depende do hashrate atual da rede. Isso é completamente normal.

### Templates com transações reais
Diferente do regtest (blocos vazios), os templates da testnet4 contêm transações reais de outros usuários. O miner as inclui no bloco automaticamente.

### Quando um bloco for encontrado
```
[INFO] ★ BLOCK FOUND ★ thread=X nonce=Y extranonce=0xZ
[INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

O bloco vai para nós reais. Busque o hash em `mempool.space/testnet4` — o coinbase terá `/btc-lottery-miner/` como tag.

### tBTC
A recompensa é tBTC — sem valor monetário, mas prova que o fluxo completo funciona antes de migrar para a mainnet.

---

## Conclusão — testnet4 validada ✅

| Item validado | Resultado |
|---|---|
| Sync completo (~10 GB) | ✅ ~2h com 4 GB RAM e `-dbcache=256` |
| Conexão à rede P2P real | ✅ Peers conectados, blocos recebidos |
| Templates com transações reais | ✅ `txs=4, value=5000003490 sat` |
| Atualização automática de template | ✅ 63 transições em 6h sem intervenção |
| Hashrate estável | ✅ ~9 MH/s contínuo |
| Nenhum erro de RPC ou rejeição inesperada | ✅ Logs limpos durante toda a execução |
| Bloco encontrado | — Não (esperado — CPU vs rede com centenas de GH/s) |

**Decisão:** fluxo completo validado em rede real. Partir para a **mainnet**.

---

## Encerrar e limpar

```powershell
# Parar o nó
bitcoin-cli -testnet4 stop

# Apagar blockchain (libera ~10 GB)
bitcoin-cli -testnet4 stop
Remove-Item -Recurse -Force "$env:APPDATA\Bitcoin\testnet4"
```

> **Executado:** nó parado via `bitcoin-cli -testnet4 stop`. Blockchain testnet4 mantida em disco para possível retomada futura.

---

## Retomar em uma nova sessão

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
Start-Process -FilePath bitcoind -ArgumentList "-testnet4 -dbcache=256" -WindowStyle Minimized
Start-Sleep -Seconds 5
bitcoin-cli -testnet4 getblockchaininfo | ConvertFrom-Json |
  Select-Object chain, blocks, verificationprogress
```

Se o sync não estava completo, retoma de onde parou automaticamente.
