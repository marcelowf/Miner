# Setup — Instalação do ambiente (Windows)

Guia para instalar todas as dependências em uma máquina Windows do zero e compilar o `btc-lottery-miner`.

**Tempo estimado:** 30–60 min (dominado por downloads)
**Espaço necessário:** ~8 GB (MSVC ~4 GB, Rust ~1 GB, Bitcoin Core ~500 MB)

---

## Pré-requisitos

- Windows 10 (build 1809+) ou Windows 11
- ~10 GB livres em disco
- Conexão de internet
- PowerShell (já instalado — abra como usuário normal, não precisa de "Run as Administrator")

### Verificar winget

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" --version
```

Esperado: `v1.x.x`. Se der erro, instale o **App Installer** pela Microsoft Store.

---

## 1. Git

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install --id Git.Git -e --accept-package-agreements --accept-source-agreements
```

**Feche e reabra o PowerShell.** Verifique:

```powershell
git --version
# git version 2.54.0.windows.1
```

---

## 2. MSVC Build Tools 2022

O Rust precisa do linker `link.exe` do Visual Studio para compilar no Windows.

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install `
  --id Microsoft.VisualStudio.2022.BuildTools -e --silent `
  --accept-package-agreements --accept-source-agreements `
  --override "--quiet --wait --norestart --nocache `
    --add Microsoft.VisualStudio.Workload.VCTools `
    --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

Baixa ~3 GB, demora 5–10 min sem feedback visual — é normal. Verifique:

```powershell
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
& $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json |
  ConvertFrom-Json | Select-Object -First 1 installationPath, installationVersion
```

---

## 3. Rust / Cargo

O `winget install Rustlang.Rustup` trava esperando input interativo. Instale manualmente:

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

Adicionar Cargo ao PATH (permanente):

```powershell
[Environment]::SetEnvironmentVariable(
  "PATH",
  "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("PATH","User"),
  "User"
)
```

Feche e reabra o PowerShell. Verifique:

```powershell
cargo --version   # cargo 1.96.0
rustc --version   # rustc 1.96.0
```

---

## 4. Bitcoin Core 31.0

### Instalar via winget

> **Nota:** O ID mudou. O ID antigo `Bitcoin.BitcoinCore` não funciona mais.

```powershell
& "$env:LOCALAPPDATA\Microsoft\WindowsApps\winget.exe" install `
  --id BitcoinCoreProject.BitcoinCore -e `
  --accept-package-agreements --accept-source-agreements
```

### Problema: bitcoind.exe não vem no instalador

O instalador `.exe` do winget instala apenas a GUI (`bitcoin-qt.exe`) e utilitários (`bitcoin-cli`, `bitcoin-tx`, `bitcoin-wallet`), **mas não inclui `bitcoind.exe`**.

**Solução:** baixar o zip completo e extrair:

```powershell
Invoke-WebRequest `
  -Uri "https://bitcoincore.org/bin/bitcoin-core-31.0/bitcoin-31.0-win64.zip" `
  -OutFile "$env:TEMP\bitcoin-31.0-win64.zip" -UseBasicParsing

Expand-Archive "$env:TEMP\bitcoin-31.0-win64.zip" `
  -DestinationPath "$env:TEMP\bitcoin-extract" -Force

# Program Files exige admin — copiar para pasta do usuário
New-Item -ItemType Directory -Force "$env:USERPROFILE\bin" | Out-Null
Copy-Item "$env:TEMP\bitcoin-extract\bitcoin-31.0\bin\bitcoind.exe" "$env:USERPROFILE\bin\bitcoind.exe"
Copy-Item "$env:TEMP\bitcoin-extract\bitcoin-31.0\bin\bitcoin-cli.exe" "$env:USERPROFILE\bin\bitcoin-cli.exe"

# PATH permanente
[Environment]::SetEnvironmentVariable(
  "PATH",
  "$env:USERPROFILE\bin;" + [Environment]::GetEnvironmentVariable("PATH","User"),
  "User"
)
```

Verifique:

```powershell
bitcoind --version    # Bitcoin Core daemon version v31.0.0
bitcoin-cli --version
```

---

## 5. Compilar o miner

```powershell
Set-Location C:\Develop\Miner
cargo build --release
```

Primeira build ~2 min (baixa e compila ~100 crates). Builds subsequentes são incrementais.

**Binário:** `target\release\btc-lottery-miner.exe` (~3 MB, sem dependências externas)

```powershell
.\target\release\btc-lottery-miner.exe --help
```

---

## PATH em cada sessão nova

O PATH persistido acima só vale após reabrir o terminal. Em sessões novas, defina manualmente se necessário:

```powershell
$env:PATH = "$env:USERPROFILE\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
```

---

## Problemas conhecidos

| Problema | Causa | Solução |
|---|---|---|
| `winget install Bitcoin.BitcoinCore` → "Nenhum pacote encontrado" | ID mudou no repositório | Usar `BitcoinCoreProject.BitcoinCore` |
| `bitcoind` não reconhecido após instalar | `bitcoind.exe` não vem no installer `.exe` | Baixar zip de bitcoincore.org |
| `Copy-Item` para `C:\Program Files\...` → acesso negado | Pasta protegida, exige admin | Copiar para `%USERPROFILE%\bin\` |
| `bitcoind -regtest -daemon` → "-daemon is not supported on this OS" | `-daemon` não existe no Windows | Usar `Start-Process` (ver [regtest.md](regtest.md)) |
| `link.exe not found` durante `cargo build` | MSVC Build Tools não instalou | Verificar com `vswhere` (seção 2) |
| `cargo: termo não reconhecido` | PATH não atualizado | Reabra o terminal ou defina manualmente |
