# Setup completo — máquina Windows zerada

Guia ponta a ponta para clonar, compilar e rodar o `btc-lottery-miner` em uma máquina Windows recém-instalada **sem nenhuma ferramenta de desenvolvimento**. Vamos do `winget` ao primeiro bloco minerado em regtest.

> **Tempo total estimado:** 30–60 min (download dominante).
> **Espaço em disco:** ~8 GB (MSVC ~4 GB, Rust ~1 GB, Bitcoin Core ~500 MB, deps + binário ~1 GB).

---

## 0. Pré-requisitos da máquina

- Windows 10 (build 1809+) ou Windows 11
- Conta de usuário com permissão de administrador (vai precisar para alguns instaladores)
- ~10 GB livres
- Conexão razoável (vamos baixar ~6 GB de instaladores)
- PowerShell — já vem instalado, abra como **usuário normal** (não precisa "Run as Administrator" para os comandos abaixo; os instaladores pedem UAC sozinhos quando necessário)

### Verifique se o winget está disponível

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" --version
```

Esperado: `v1.x.x`. Se der erro de "não reconhecido", instale o **App Installer** da Microsoft Store antes de continuar:
https://apps.microsoft.com/detail/9NBLGGH4NNS1

---

## 1. Instalar Git

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install --id Git.Git -e --accept-package-agreements --accept-source-agreements
```

**Feche e reabra o PowerShell.** Verifique:

```powershell
git --version
```

---

## 2. Instalar MSVC Build Tools (linker que o Rust usa)

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install `
  --id Microsoft.VisualStudio.2022.BuildTools -e `
  --silent --accept-package-agreements --accept-source-agreements `
  --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

Demora 5–10 min, baixa ~3 GB. **Não dá feedback de progresso no PowerShell** — é normal, ele tá baixando + instalando em silencio. Espere terminar (volta o prompt).

Verifique:

```powershell
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
& $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json |
  ConvertFrom-Json | Select-Object -First 1 installationPath, installationVersion
```

Deve mostrar o caminho do BuildTools instalado.

---

## 3. Instalar Rust (rustup + cargo + rustc)

O `winget install Rustlang.Rustup` roda o instalador **interativo** e trava esperando input. Por isso baixamos e rodamos manualmente com flags unattended:

```powershell
Invoke-WebRequest `
  -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" `
  -OutFile "$env:TEMP\rustup-init.exe" -UseBasicParsing

& "$env:TEMP\rustup-init.exe" -y `
  --default-toolchain stable `
  --default-host x86_64-pc-windows-msvc `
  --profile minimal `
  --no-modify-path
```

Baixa ~300 MB. Demora 1–3 min.

---

## 4. Adicionar Cargo ao PATH (uma vez só)

Como instalamos com `--no-modify-path`, precisamos adicionar manualmente. Para a **sessão atual**:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

Para **persistir entre terminais** (registro do Windows, usuário atual):

```powershell
[Environment]::SetEnvironmentVariable(
  "PATH",
  "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("PATH","User"),
  "User"
)
```

Feche e reabra o PowerShell. Verifique:

```powershell
cargo --version
rustc --version
```

Esperado: `cargo 1.95.x` e `rustc 1.95.x` (ou mais novo).

---

## 5. Clonar o repositório

Escolha onde quer guardar o projeto (vou usar `C:\dev` como exemplo):

```powershell
New-Item -ItemType Directory -Force C:\dev | Out-Null
Set-Location C:\dev
git clone https://github.com/marcelowf/Miner.git
Set-Location Miner
```

> **Para mim (Marcelo):** se você está restaurando seu próprio repo, esse é o URL. Para alguém clonando o projeto pela primeira vez, é o mesmo URL público.

---

## 6. Compilar o miner

```powershell
cargo build --release
```

Primeira build leva ~2 min (baixa 100+ crates e compila tudo). Cargo guarda o cache em `target/` — buildos subsequentes são incrementais (1–10s).

Resultado: `target\release\btc-lottery-miner.exe` (~3 MB, single-file, sem dependências externas).

Sanity check:

```powershell
.\target\release\btc-lottery-miner.exe --help
```

---

## 7. Instalar Bitcoin Core

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install --id Bitcoin.BitcoinCore -e --accept-package-agreements --accept-source-agreements
```

Adicione ao PATH da sessão:

```powershell
$env:PATH = "C:\Program Files\Bitcoin\daemon;$env:PATH"
bitcoind --version
```

> ### ⚠️ AVISO CRÍTICO sobre o nó
>
> Bitcoin Core, **rodado sem flag**, assume **mainnet** e começa a baixar **~700 GB** silenciosamente. Você não quer isso agora.
>
> | Modo | Download | Disco | Tempo |
> |---|---|---|---|
> | **regtest** (este guia) | 0 GB | ~50 MB | instantâneo |
> | mainnet (default!) | ~700 GB | ~700 GB | 2–5 dias |
>
> Os comandos abaixo **sempre** passam `-regtest`. Faça o mesmo. Nunca rode `bitcoind` solto.

---

## 8. Configurar o nó em regtest

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

Deve ecoar o conteúdo do arquivo.

---

## 9. Subir o nó em regtest

```powershell
bitcoind -regtest -daemon
```

Espere ~3 segundos e verifique:

```powershell
bitcoin-cli -regtest getblockchaininfo
```

Esperado: `"chain": "regtest", "blocks": 0`. Se der `Could not connect`, espere mais 2s e tente de novo (o nó leva um instante para abrir a porta RPC).

---

## 10. Criar a carteira de payout e pegar o `scriptPubKey`

```powershell
bitcoin-cli -regtest createwallet "lottery"
$addr = bitcoin-cli -regtest -rpcwallet=lottery getnewaddress "" "bech32"
$script = (bitcoin-cli -regtest -rpcwallet=lottery getaddressinfo $addr | ConvertFrom-Json).scriptPubKey
"Endereço: $addr"
"Script:   $script"
```

Guarde o `$script` — é ele que vai no `config.toml`. Tem cara de `0014<40 chars hex>` (P2WPKH = `OP_0` + push de 20 bytes do hash da pubkey).

> Para mainnet de verdade, você gera o endereço numa carteira séria (Sparrow, Electrum, hardware wallet) e usa o scriptPubKey **daquele** endereço. Em regtest, o BTC é fake — usar a wallet do próprio nó é o jeito mais limpo.

---

## 11. Criar `config.toml`

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
$cfg | Out-File "config.toml" -Encoding ascii
Get-Content config.toml
```

Confira que `payout_script_hex` foi substituído pelo hex (não está literalmente `$script`).

---

## 12. Rodar o miner

```powershell
.\target\release\btc-lottery-miner.exe --config config.toml
```

Esperado em segundos:

```
[INFO] btc-lottery-miner v0.1.0
[INFO] Connected to node — chain=regtest, height=0
[INFO] Initial template: height=1, txs=0, value=5000000000 sat
[INFO] Mining with 16 threads
[INFO]  142.30 MH/s | hashes=711500000
[INFO] ★ BLOCK FOUND ★ thread=3 nonce=2147483921 extranonce=0x300000007
[INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

`Ctrl+C` para parar. Em regtest a dificuldade é mínima — vai achar bloco quase imediatamente.

---

## 13. Verificar que o bloco foi aceito

```powershell
bitcoin-cli -regtest getblockchaininfo
```

`blocks` deve ter incrementado.

Inspecione o bloco que **você** minerou:

```powershell
$hash = bitcoin-cli -regtest getbestblockhash
bitcoin-cli -regtest getblock $hash 2 | ConvertFrom-Json | Select-Object hash, height, nTx, @{N='coinbase';E={$_.tx[0].vin[0].coinbase}}
```

O campo `coinbase` vai conter `/btc-lottery-miner/` em ASCII hex (`2f6274632d6c6f74746572792d6d696e65722f`). Isso prova que foi **o seu binário** que produziu o bloco.

Confira o saldo da carteira:

```powershell
bitcoin-cli -regtest -rpcwallet=lottery getbalances
```

Vai mostrar `immature: 50.00000000` — em Bitcoin coinbase só matura após 100 blocos. Para "envelhecer" instantaneamente:

```powershell
bitcoin-cli -regtest generatetoaddress 100 $addr
bitcoin-cli -regtest -rpcwallet=lottery getbalances
```

Agora `trusted` mostra o saldo confirmado.

---

## 14. Encerrar o nó (quando quiser parar)

```powershell
bitcoin-cli -regtest stop
```

Para resetar o estado da regtest (apagar a "blockchain" que você criou):

```powershell
bitcoin-cli -regtest stop
Remove-Item -Recurse -Force "$env:APPDATA\Bitcoin\regtest"
```

Na próxima vez que rodar `bitcoind -regtest -daemon`, começa do bloco 0 de novo.

---

## Troubleshooting

| Sintoma | Causa provável / fix |
|---|---|
| `winget: termo não reconhecido` | App Installer não instalado. Veja seção 0. |
| `cargo: termo não reconhecido` | PATH não atualizado. Reabra o PowerShell ou rode `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`. |
| `link.exe not found` durante `cargo build` | MSVC Build Tools não instalou direito. Volte à seção 2 e confirme com o `vswhere`. |
| `bitcoind: termo não reconhecido` | PATH não tem `C:\Program Files\Bitcoin\daemon`. Adicione na sessão (`$env:PATH = "C:\Program Files\Bitcoin\daemon;$env:PATH"`). |
| `Could not connect to the server 127.0.0.1:18443` | Nó não está rodando ou ainda não terminou de subir. Aguarde 3s e tente de novo. Confira `Get-Process bitcoind`. |
| `401 Unauthorized` ao chamar `bitcoin-cli` | user/password no `bitcoin.conf` não batem com o `config.toml` (ou com o que o cli espera). Reveja seções 8 e 11. |
| Miner trava sem achar bloco em regtest | Impossível em condições normais (target trivial). Se acontecer, mostre os logs em `--log debug` — é bug nosso, corrigimos. |
| `submitblock rejected: bad-cb-amount` ou `bad-witness-merkle-match` | Bug de montagem do bloco. Capture a mensagem exata e reporte. |
| `cargo build` reclama de OpenSSL ou perl | Não deve acontecer — nossas deps são pure-Rust. Se acontecer, conferir se `Cargo.toml` foi alterado. |
| Antivírus bloqueia `btc-lottery-miner.exe` | Adicione exceção. Binário de mineração frequentemente é falso-positivo. |

---

## Apêndice A — Recompilar após editar código

```powershell
cargo build --release
```

Apenas os arquivos alterados (e seus dependentes) recompilam.

Para uma build mais rápida sem otimizações (uso de debug):

```powershell
cargo run -- --config config.toml
```

(Roda direto, sem precisar copiar o binário. Bem mais lento em runtime — 5–10x menos hashrate — mas compila em segundos.)

---

## Apêndice B — Limpar tudo

Para começar do zero sem desinstalar nada:

```powershell
bitcoin-cli -regtest stop
Remove-Item -Recurse -Force "$env:APPDATA\Bitcoin"
Set-Location C:\dev\Miner
cargo clean
```

Para desinstalar Rust:
```powershell
rustup self uninstall
```

Para desinstalar Bitcoin Core e Build Tools, use **Configurações → Aplicativos** do Windows.

---

## Apêndice C — Roteiro resumido (TL;DR)

Para a próxima vez que você precisar refazer isso e quiser pular as explicações:

```powershell
# 1. Pré-reqs
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install Git.Git -e --silent --accept-package-agreements --accept-source-agreements
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install Microsoft.VisualStudio.2022.BuildTools -e --silent --accept-package-agreements --accept-source-agreements --override "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
Invoke-WebRequest "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile "$env:TEMP\rustup-init.exe" -UseBasicParsing
& "$env:TEMP\rustup-init.exe" -y --default-toolchain stable --default-host x86_64-pc-windows-msvc --profile minimal --no-modify-path
[Environment]::SetEnvironmentVariable("PATH", "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("PATH","User"), "User")
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install Bitcoin.BitcoinCore -e --silent --accept-package-agreements --accept-source-agreements

# >>> Feche e reabra o PowerShell aqui (PATH precisa propagar) <<<

# 2. Build
New-Item -ItemType Directory -Force C:\dev | Out-Null
Set-Location C:\dev
git clone https://github.com/marcelowf/Miner.git
Set-Location Miner
cargo build --release

# 3. Nó
$env:PATH = "C:\Program Files\Bitcoin\daemon;$env:PATH"
@"
regtest=1
server=1
[regtest]
rpcuser=miner
rpcpassword=lottery
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
fallbackfee=0.0001
"@ | Out-File "$env:APPDATA\Bitcoin\bitcoin.conf" -Encoding ascii
bitcoind -regtest -daemon
Start-Sleep -Seconds 3

# 4. Wallet + config
bitcoin-cli -regtest createwallet "lottery"
$addr = bitcoin-cli -regtest -rpcwallet=lottery getnewaddress "" "bech32"
$script = (bitcoin-cli -regtest -rpcwallet=lottery getaddressinfo $addr | ConvertFrom-Json).scriptPubKey
@"
[rpc]
url = "http://127.0.0.1:18443"
user = "miner"
password = "lottery"

[mining]
payout_script_hex = "$script"
threads = 0
refresh_seconds = 30
coinbase_tag = "/btc-lottery-miner/"
"@ | Out-File "config.toml" -Encoding ascii

# 5. Minerar
.\target\release\btc-lottery-miner.exe --config config.toml
```

---

## O que NÃO está coberto aqui

- **Setup mainnet.** Vai precisar de SSD dedicado, configurações de pruning, carteira séria (Sparrow/hardware wallet), e dias de IBD. Quando quiser, fazemos um `MAINNET.md` separado.
- **Distribuição.** Como publicar binários em GitHub Releases via Actions para Windows/Linux/Mac. Item do roadmap no `README.md`.
- **GPU/ASIC mining.** Fora de escopo — este projeto é educacional, CPU-only.
