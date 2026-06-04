# Regtest — Mineração local

Guia completo para rodar o `btc-lottery-miner` em regtest — ambiente local isolado, sem internet, blocos encontrados em segundos.

**Pré-requisito:** ambiente instalado conforme [setup.md](setup.md)

---

## O que é regtest

| Característica | Valor |
|---|---|
| Rede | local, completamente isolado |
| Blockchain | você cria do zero (0 blocos) |
| Bloco encontrado | segundos (dificuldade mínima) |
| BTC | falso, sem valor |
| Sync necessário | não |
| Uso | desenvolvimento, testes, aprendizado |

Ideal para testar o fluxo completo sem depender de rede ou esperar sync.

---

## 1. Configurar bitcoin.conf

**Localização:** `%APPDATA%\Bitcoin\bitcoin.conf`

```powershell
$conf = @'
regtest=1
server=1
[regtest]
rpcuser=miner
rpcpassword=lottery
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
fallbackfee=0.0001
'@
New-Item -ItemType Directory -Force "$env:APPDATA\Bitcoin" | Out-Null
$conf | Out-File "$env:APPDATA\Bitcoin\bitcoin.conf" -Encoding ascii
Get-Content "$env:APPDATA\Bitcoin\bitcoin.conf"
```

---

## 2. Iniciar o nó

> **Windows não suporta `-daemon`.** Use `Start-Process` para rodar em background.

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:PATH"
Start-Process -FilePath bitcoind -ArgumentList "-regtest" -WindowStyle Minimized
Start-Sleep -Seconds 3
bitcoin-cli -regtest getblockchaininfo
```

Esperado: `"chain": "regtest"`, `"blocks": 0`

---

## 3. Carteira e config.toml

Rode os dois blocos **na mesma sessão PowerShell** — `$script` precisa estar na memória quando criar o config.toml.

```powershell
bitcoin-cli -regtest createwallet "lottery"
$addr = bitcoin-cli -regtest -rpcwallet=lottery getnewaddress "" "bech32"
$script = (bitcoin-cli -regtest -rpcwallet=lottery getaddressinfo $addr | ConvertFrom-Json).scriptPubKey
"Endereço: $addr"
"Script:   $script"
```

Resultado real obtido:
- Endereço: `bcrt1qwmlcum4lgjzucw0794mu3pyk58lwwfta398fh7` (prefixo `bcrt1` = regtest)
- scriptPubKey: `001476ff8e6ebf4485cc39fe2d77c88496a1fee7257d` (P2WPKH)

```powershell
$cfg = @"
[rpc]
url = "http://127.0.0.1:18443"
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

Confirme que `payout_script_hex` mostra o hex real (`0014...`), não o texto `$script`.

---

## 4. Rodar o miner

```powershell
Set-Location C:\Develop\Miner
.\target\release\btc-lottery-miner.exe --config config.toml
```

Em regtest a dificuldade é mínima — bloco encontrado em segundos. `Ctrl+C` para parar.

> Os múltiplos `submitblock rejected: inconclusive` são **normais**: 16 threads acham blocos válidos quase simultaneamente para a mesma altura. O nó aceita o primeiro e devolve `inconclusive` para os demais.

---

## Bug corrigido: `bad-cb-height`

### Sintoma
Antes do fix, todos os blocos eram rejeitados:
```
★ BLOCK FOUND ★ thread=3 nonce=0 extranonce=0x300000042
submitblock rejected: bad-cb-height
```

### Causa
BIP34 exige que o coinbase scriptSig comece com um push da altura do bloco. Bitcoin Core valida isso comparando os bytes literais do scriptSig com `CScript() << nHeight`, que para alturas **1–16** usa opcodes `OP_1`–`OP_16` (um único byte `0x51`–`0x60`).

O código original usava CScriptNum encoding — um length byte + value byte — que diverge para alturas baixas:

| Altura | Esperado (Bitcoin Core) | Gerado antes do fix |
|---|---|---|
| 1 | `51` (OP_1) | `01 01` |
| 2 | `52` (OP_2) | `01 02` |
| 16 | `60` (OP_16) | `01 10` |
| 17 | `01 11` | `01 11` ✓ |

Em regtest o primeiro bloco tem sempre altura 1 — o bug era 100% reproduzível.

### Fix — `src/coinbase.rs`

```rust
// ANTES (bugado para heights 1–16):
fn push_bip34_height(buf: &mut Vec<u8>, height: u32) {
    let mut n = height as i64;
    let mut bytes = Vec::with_capacity(5);
    let negative = n < 0;
    if negative { n = -n; }
    while n != 0 {
        bytes.push((n & 0xff) as u8);
        n >>= 8;
    }
    if let Some(&last) = bytes.last() {
        if last & 0x80 != 0 {
            bytes.push(if negative { 0x80 } else { 0x00 });
        } else if negative {
            *bytes.last_mut().unwrap() |= 0x80;
        }
    }
    buf.push(bytes.len() as u8);
    buf.extend_from_slice(&bytes);
}

// DEPOIS (correto):
fn push_bip34_height(buf: &mut Vec<u8>, height: u32) {
    match height {
        0 => buf.push(0x00),                     // OP_0 — genesis, nunca minerado
        1..=16 => buf.push(0x50 + height as u8), // OP_1 (0x51) .. OP_16 (0x60)
        _ => {
            // CScriptNum minimal encoding para height >= 17
            let mut n = height;
            let mut bytes: Vec<u8> = Vec::with_capacity(4);
            while n > 0 {
                bytes.push((n & 0xff) as u8);
                n >>= 8;
            }
            if bytes.last().copied().unwrap_or(0) & 0x80 != 0 {
                bytes.push(0x00);
            }
            buf.push(bytes.len() as u8);
            buf.extend_from_slice(&bytes);
        }
    }
}
```

---

## Resultado — bloco 1 minerado

### Output do miner
```
[INFO] btc-lottery-miner v0.1.0
[INFO] Connected to node — chain=regtest, height=0
[INFO] Initial template: height=1, txs=0, value=5000000000 sat
[INFO] Mining with 16 threads
[INFO] ★ BLOCK FOUND ★ thread=0 nonce=0 extranonce=0x0
[INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

### Verificação on-chain

```powershell
bitcoin-cli -regtest getblockchaininfo
# "blocks": 1

$hash = bitcoin-cli -regtest getbestblockhash
bitcoin-cli -regtest getblock $hash 2 | ConvertFrom-Json |
  Select-Object hash, height, @{N='coinbase';E={$_.tx[0].vin[0].coinbase}}
```

Resultado real:
```
hash    : 35f556e57465102af06ba85713c384f2823f9040b1e6852b65074873ff811a3a
height  : 1
coinbase: 510100132f6274632d6c6f74746572792d6d696e65722f
```

### Decode do coinbase scriptSig

| Bytes | Significado |
|---|---|
| `51` | OP_1 — BIP34 height = 1 (fix aplicado) |
| `01 00` | extranonce = 0 |
| `13 2f6274632d6c6f74746572792d6d696e65722f` | `/btc-lottery-miner/` (19 bytes) |

---

## Maturação da recompensa

Coinbase requer 100 confirmações para ser gasto. Mine 100 blocos extras para maturar:

```powershell
$addr = bitcoin-cli -regtest -rpcwallet=lottery getnewaddress "" "bech32"
bitcoin-cli -regtest generatetoaddress 100 $addr
bitcoin-cli -regtest -rpcwallet=lottery getbalances
```

Resultado:
```json
{
  "mine": {
    "trusted": 50.00000000,
    "immature": 5000.00000000
  }
}
```

- `trusted: 50` — recompensa do bloco minerado, confirmada e gastável
- `immature: 5000` — recompensa dos 100 blocos do `generatetoaddress`
- Blockchain na altura 101

---

## Encerrar e resetar

```powershell
# Parar o nó
bitcoin-cli -regtest stop

# Resetar a blockchain (volta ao bloco 0)
bitcoin-cli -regtest stop
Remove-Item -Recurse -Force "$env:APPDATA\Bitcoin\regtest"
```

---

## Retomar em uma nova sessão

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
Start-Process -FilePath bitcoind -ArgumentList "-regtest" -WindowStyle Minimized
Start-Sleep -Seconds 3
bitcoin-cli -regtest getblockchaininfo
Set-Location C:\Develop\Miner
.\target\release\btc-lottery-miner.exe --config config.toml
```
