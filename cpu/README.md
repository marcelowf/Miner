# Bitcoin CPU Miner — Loteria de Bloco

Minerador de Bitcoin em CPU que se conecta a um nó (Bitcoin Core) via RPC e tenta achar um bloco "na sorte".

> Sim, isso é uma **loteria**. Em 2026 a dificuldade da mainnet exige ASICs na casa de centenas de TH/s; uma CPU faz alguns MH/s. A probabilidade de uma CPU achar um bloco antes do calor da morte do universo é desprezível — mas **não é zero**. Cada hash tem a mesma chance de qualquer outro hash. É exatamente isso que torna o projeto interessante: bilhete de loteria com recompensa de ~3.125 BTC + fees, comprado em ciclos de CPU.

---

## O que esse projeto faz

1. Conecta a um nó Bitcoin (local ou remoto) via JSON-RPC.
2. Pede um template de bloco com `getblocktemplate`.
3. Monta o header de 80 bytes e itera o `nonce`/`extranonce`.
4. Calcula `SHA256(SHA256(header))` em loop.
5. Se `hash < target` → chama `submitblock` e abre o champagne.
6. Senão → pede novo template (a cada N segundos ou quando chega bloco novo) e repete.

Funciona em três modos:

| Modo | Para que serve |
|---|---|
| **regtest** | Aprender. Acha bloco em milissegundos. Recompensa é fake. |
| **testnet** | Sentir o protocolo real sem queimar dinheiro. Recompensa é tBTC (sem valor). |
| **mainnet** | A loteria de verdade. Roda 24/7 e reza. |

---

## Pré-requisitos

- **Bitcoin Core** instalado e sincronizado (ou acesso RPC a um nó de terceiro).
  - Mainnet sincronizado ocupa ~700 GB. Se você não quer manter o nó, pode usar um endpoint público de terceiros (cuidado com confiança — quem te entrega o template controla o que você minera).
- **Rust toolchain** (`rustup` + `cargo`). Veja **Build do zero (Windows)** abaixo.
- Uma CPU. Qualquer uma. Quanto mais núcleos, mais bilhetes por segundo.

---

## Build do zero (Windows)

### 1. Build Tools do Visual Studio (linker do MSVC)

O Rust no Windows usa o linker do MSVC. Se você já tem Visual Studio com workload "Desktop development with C++", pode pular. Caso contrário:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent `
  --override "--quiet --wait --norestart --nocache `
  --add Microsoft.VisualStudio.Workload.VCTools `
  --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
  --add Microsoft.VisualStudio.Component.Windows11SDK.22621"
```

### 2. Rustup + toolchain

```powershell
Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" `
  -OutFile "$env:TEMP\rustup-init.exe" -UseBasicParsing
& "$env:TEMP\rustup-init.exe" -y --default-toolchain stable `
  --default-host x86_64-pc-windows-msvc --profile minimal --no-modify-path
```

### 3. ⚠️ Adicionar `cargo` ao PATH (uma vez só)

O comando acima usa `--no-modify-path` para não mexer no PATH global sem permissão. Para `cargo` funcionar em qualquer terminal novo, persista no PATH do usuário:

```powershell
[Environment]::SetEnvironmentVariable(
  "PATH",
  "$env:USERPROFILE\.cargo\bin;" + [Environment]::GetEnvironmentVariable("PATH","User"),
  "User"
)
```

Feche e reabra o terminal. Verifique: `cargo --version`.

> **Alternativa rápida (só na sessão atual, não persiste):**
> `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`

### 4. Compilar

```powershell
cargo build --release
```

Primeira build leva ~2 min (baixa e compila deps). Binário final: `target\release\btc-lottery-miner.exe` (~3 MB, single-file, sem runtime).

---

## Configurando o nó

`bitcoin.conf`:

```ini
# Escolha UM dos três:
# regtest=1
# testnet=1
# (vazio para mainnet)

server=1
rpcuser=miner
rpcpassword=trocar_isto_por_algo_forte
rpcallowip=127.0.0.1
rpcbind=127.0.0.1

# Mantém o mempool cheio para incluir fees no bloco
blockmaxweight=4000000
```

Suba o nó:

```powershell
bitcoind -daemon
```

Cheque que o RPC responde:

```powershell
bitcoin-cli getblockchaininfo
```

---

## Configuração do minerador

Copie `config.example.toml` para `config.toml` e edite:

```toml
[rpc]
url = "http://127.0.0.1:18443"   # regtest=18443, testnet=18332, mainnet=8332
user = "miner"
password = "trocar_isto_por_algo_forte"

[mining]
# scriptPubKey hex da sua carteira (NÃO o endereço bech32).
# Pegue com: bitcoin-cli -regtest getaddressinfo $(bitcoin-cli -regtest getnewaddress) | jq -r .scriptPubKey
payout_script_hex = "0014..."
threads = 0                       # 0 = todos os núcleos lógicos
refresh_seconds = 30
coinbase_tag = "/btc-lottery-miner/"
```

- `payout_script_hex`: scriptPubKey de uma carteira **que você controla as chaves privadas**.
- `threads`: `0` = todos os núcleos.
- `refresh_seconds`: a cada quantos segundos pedir novo template. Se sair um bloco novo na rede e você não atualizar, está minerando em cima do bloco errado (= queimando ciclos).

---

## Como rodar

```powershell
.\target\release\btc-lottery-miner.exe --config config.toml
```

Para regtest (mais rápido para validar):
```powershell
.\target\release\btc-lottery-miner.exe --config config.toml --log debug
```

Saída esperada:

```
[2026-05-25T12:00:01Z INFO] btc-lottery-miner v0.1.0
[2026-05-25T12:00:01Z INFO] Connected to node — chain=regtest, height=0
[2026-05-25T12:00:01Z INFO] Initial template: height=1, txs=0, value=5000000000 sat
[2026-05-25T12:00:01Z INFO] Mining with 16 threads
[2026-05-25T12:00:06Z INFO]  142.30 MH/s | hashes=711500000
[2026-05-25T12:00:11Z INFO] ★ BLOCK FOUND ★ thread=3 nonce=2147483921 extranonce=0x300000007
[2026-05-25T12:00:11Z INFO] submitblock OK — you won the lottery. Wait 100 confirmations.
```

Pare com `Ctrl+C`. O processo é stateless — pode parar e voltar à vontade.

---

## Arquitetura

```
┌─────────────────┐  getblocktemplate   ┌──────────────────┐
│                 │ ──────────────────► │                  │
│  Miner (este)   │                     │  Bitcoin Core    │
│                 │ ◄────────────────── │  (seu nó)        │
└────────┬────────┘    submitblock      └──────────────────┘
         │
         │ spawn N threads
         ▼
┌─────────────────────────────────────────┐
│  Worker[i]: itera nonces num range      │
│  header = ver|prev|merkle|time|bits|n   │
│  if dsha256(header) < target → submit   │
└─────────────────────────────────────────┘
```

**Particionamento do espaço de busca:** cada thread recebe um `extranonce` distinto (modifica a coinbase, força um merkle root diferente) e varre seus 2³² nonces independentemente. Sem sincronização entre threads no hot loop — só checa uma flag atômica `templateExpired`.

---

## O hot loop (essência)

```csharp
Span<byte> header = stackalloc byte[80];
BuildHeader(header, version, prevHash, merkleRoot, time, bits);

uint nonce = startNonce;
while (!templateExpired)
{
    BinaryPrimitives.WriteUInt32LittleEndian(header[76..], nonce);

    Span<byte> h1 = stackalloc byte[32];
    Span<byte> h2 = stackalloc byte[32];
    SHA256.HashData(header, h1);
    SHA256.HashData(h1, h2);

    if (IsBelowTarget(h2, target))
    {
        SubmitBlock(header, coinbaseTx, otherTxs);
        return;
    }
    nonce++;
}
```

Sem otimizações exóticas (SIMD, AVX2 midstate caching, GPU). O objetivo é **legibilidade**, não competir com `cpuminer-opt`. Se quiser velocidade séria, troque `SHA256.HashData` por uma rotina vetorizada que cacheia o midstate dos primeiros 64 bytes do header (só os últimos 16 bytes mudam com o nonce).

---

## Custos honestos

| Item | Valor |
|---|---|
| Consumo de uma CPU desktop em load total | ~100 W |
| Custo de 1 mês 24/7 a R$ 0,80/kWh | ~R$ 58 |
| Hashrate típico (CPU moderna, 16 threads) | ~150 MH/s |
| Hashrate da rede em 2026 | ~700 EH/s |
| Sua fração da rede | ~2 × 10⁻¹³ |
| Blocos por dia na rede | 144 |
| **Sua expectativa de blocos por mês** | **~9 × 10⁻¹⁰** |
| Em anos para 1 bloco esperado | ~93 milhões |

Comprar Mega-Sena tem expected value pior, mas pelo menos é mais barato.

---

## FAQ

**Por que não usar uma pool?**
Porque pool não é loteria — é renda proporcional. Em pool sua CPU recebe migalhas constantes em vez de um prêmio improvável. Esse projeto é deliberadamente **solo mining**: ou tudo ou nada. Se quiser pool, use [cpuminer](https://github.com/pooler/cpuminer) com Stratum.

**E se eu achar um bloco?**
A coinbase paga no endereço configurado. Você precisa esperar **100 confirmações** (≈16h) antes de poder gastar — é uma regra do protocolo, coinbases maduram devagar. Depois disso, é sua.

**Posso minerar com a CPU enquanto uso o computador?**
Pode, mas configure `Threads` para deixar 1–2 núcleos livres ou o sistema vai engasgar. Considere também: ventoinha gritando, CPU a 90°C por anos não é grátis.

**Mainnet ou testnet primeiro?**
Sempre teste em **regtest** primeiro (você vê o `submitblock` funcionando em segundos), depois **testnet** (valida o fluxo de rede real), só então mainnet.

**O nó precisa estar 100% sincronizado?**
Para mainnet, sim. Minerar em cima de um nó dessincronizado = minerar uma fork morta = trabalho jogado fora.

---

## Roadmap

- [ ] Implementação base single-threaded (regtest)
- [ ] Multi-threading com particionamento de extranonce
- [ ] Refresh automático de template ao detectar novo bloco (via ZMQ `hashblock`)
- [ ] Métricas: hashrate, melhor hash, tempo desde último template
- [ ] Suporte a Stratum (caso mude de ideia sobre pool)
- [ ] SHA256 midstate caching

---

## Referências

- [BIP 22 — getblocktemplate](https://github.com/bitcoin/bips/blob/master/bip-0022.mediawiki)
- [BIP 23 — getblocktemplate extensions](https://github.com/bitcoin/bips/blob/master/bip-0023.mediawiki)
- Ken Shirriff — *Mining Bitcoin with pencil and paper*
- `bitcoin/src/miner.cpp` no código do Bitcoin Core
- [cpuminer](https://github.com/pooler/cpuminer) — referência de implementação rápida em C

---

## Licença

MIT. É um bilhete de loteria, faça o que quiser com ele.
