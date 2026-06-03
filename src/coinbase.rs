use crate::merkle::sha256d;

/// Bitcoin-style varint (CompactSize).
pub fn write_varint(buf: &mut Vec<u8>, n: u64) {
    if n < 0xfd {
        buf.push(n as u8);
    } else if n <= 0xffff {
        buf.push(0xfd);
        buf.extend_from_slice(&(n as u16).to_le_bytes());
    } else if n <= 0xffff_ffff {
        buf.push(0xfe);
        buf.extend_from_slice(&(n as u32).to_le_bytes());
    } else {
        buf.push(0xff);
        buf.extend_from_slice(&n.to_le_bytes());
    }
}

/// BIP34: coinbase scriptSig must begin with the block height as a minimally-encoded push.
fn push_bip34_height(buf: &mut Vec<u8>, height: u32) {
    // CScriptNum minimal encoding
    let mut n = height as i64;
    let mut bytes = Vec::with_capacity(5);
    let negative = n < 0;
    if negative { n = -n; }
    while n != 0 {
        bytes.push((n & 0xff) as u8);
        n >>= 8;
    }
    // If high bit set, append 0x80 (or 0x00 for positive) so the sign bit is unambiguous.
    if let Some(&last) = bytes.last() {
        if last & 0x80 != 0 {
            bytes.push(if negative { 0x80 } else { 0x00 });
        } else if negative {
            *bytes.last_mut().unwrap() |= 0x80;
        }
    } else {
        // height = 0 — push OP_0 (already empty).
    }
    buf.push(bytes.len() as u8);
    buf.extend_from_slice(&bytes);
}

/// Builds a coinbase transaction. Returns the serialized tx in two flavors:
/// - `legacy_serialization`: NO witness data, used to compute the **txid** (which is what the
///   block merkle root hashes over).
/// - `full_serialization`: WITH witness marker/flag and witness stack, used as the actual
///   tx bytes in the block.
///
/// `extranonce` is a per-thread varying value that changes the coinbase txid, which
/// changes the merkle root, which gives each thread its own ~2^32 nonce space.
///
/// `witness_commitment_script` is the `scriptPubKey` to include as a second output containing
/// `OP_RETURN 0x24 0xaa21a9ed <wtxid_merkle_root_hash>` — required by BIP141 when the block
/// contains any segwit tx. Pass `None` for blocks with no witness txs (rare).
pub struct CoinbaseTx {
    pub legacy_serialization: Vec<u8>,
    pub full_serialization: Vec<u8>,
    pub txid: [u8; 32],
}

pub struct CoinbaseParams<'a> {
    pub height: u32,
    pub coinbase_value: u64,
    pub payout_script: &'a [u8],
    pub witness_commitment_script: Option<&'a [u8]>,
    pub extranonce: u64,
    pub tag: &'a [u8],
}

pub fn build_coinbase(p: &CoinbaseParams) -> CoinbaseTx {
    // ---- scriptSig: BIP34 height push + extranonce push + tag bytes ----
    let mut script_sig = Vec::with_capacity(64);
    push_bip34_height(&mut script_sig, p.height);

    // extranonce: minimally-encoded u64 push
    let mut extra = Vec::with_capacity(8);
    let mut n = p.extranonce;
    while n != 0 {
        extra.push((n & 0xff) as u8);
        n >>= 8;
    }
    if extra.is_empty() { extra.push(0); }
    script_sig.push(extra.len() as u8);
    script_sig.extend_from_slice(&extra);

    // Tag (free-form bytes — clamp so total scriptSig stays <=100 bytes per consensus).
    let tag_room = 100usize.saturating_sub(script_sig.len() + 1);
    let tag_slice = &p.tag[..p.tag.len().min(tag_room)];
    if !tag_slice.is_empty() {
        script_sig.push(tag_slice.len() as u8);
        script_sig.extend_from_slice(tag_slice);
    }

    // ---- Outputs ----
    // out[0]: payout
    let mut outputs = Vec::with_capacity(2);
    outputs.push((p.coinbase_value, p.payout_script.to_vec()));
    // out[1]: witness commitment (if any segwit tx present in the block)
    if let Some(wc) = p.witness_commitment_script {
        outputs.push((0u64, wc.to_vec()));
    }

    let has_witness = p.witness_commitment_script.is_some();

    // ---- Legacy (no-witness) serialization → for txid computation ----
    let legacy = serialize_tx(&script_sig, &outputs, false);
    let txid = sha256d(&legacy);

    // ---- Full serialization (with witness marker/flag + coinbase witness) ----
    let full = if has_witness {
        serialize_tx(&script_sig, &outputs, true)
    } else {
        legacy.clone()
    };

    CoinbaseTx {
        legacy_serialization: legacy,
        full_serialization: full,
        txid,
    }
}

fn serialize_tx(script_sig: &[u8], outputs: &[(u64, Vec<u8>)], with_witness: bool) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    // version
    buf.extend_from_slice(&2i32.to_le_bytes());
    // witness marker + flag
    if with_witness {
        buf.push(0x00);
        buf.push(0x01);
    }
    // input count = 1
    write_varint(&mut buf, 1);
    // input: prev_hash (32 zero), prev_index = 0xffffffff, scriptSig, sequence
    buf.extend_from_slice(&[0u8; 32]);
    buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    write_varint(&mut buf, script_sig.len() as u64);
    buf.extend_from_slice(script_sig);
    buf.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    // output count
    write_varint(&mut buf, outputs.len() as u64);
    for (value, script) in outputs {
        buf.extend_from_slice(&value.to_le_bytes());
        write_varint(&mut buf, script.len() as u64);
        buf.extend_from_slice(script);
    }
    // witness stack for coinbase input: 1 item, 32 zero bytes (BIP141 witness reserved value)
    if with_witness {
        write_varint(&mut buf, 1); // stack size
        write_varint(&mut buf, 32); // item length
        buf.extend_from_slice(&[0u8; 32]);
    }
    // locktime
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf
}
