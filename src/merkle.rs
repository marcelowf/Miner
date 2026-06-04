use sha2::{Digest, Sha256};

#[inline]
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    let mut out = [0u8; 32];
    out.copy_from_slice(&second);
    out
}

#[inline]
pub fn sha256d_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(a);
    buf[32..].copy_from_slice(b);
    sha256d(&buf)
}

/// Compute a Bitcoin merkle root from leaves already in **internal byte order**
/// (i.e. NOT reversed display order). Odd leaves at any level are duplicated.
pub fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    assert!(!leaves.is_empty(), "merkle root needs at least 1 leaf");
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            let last = *level.last().unwrap();
            level.push(last);
        }
        let mut next = Vec::with_capacity(level.len() / 2);
        for chunk in level.chunks(2) {
            next.push(sha256d_pair(&chunk[0], &chunk[1]));
        }
        level = next;
    }
    level[0]
}

/// Reverse byte order — converts display-order hashes (as RPC returns them)
/// to internal-order (as the protocol hashes them), and vice versa.
#[inline]
pub fn reverse(h: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = h[31 - i];
    }
    out
}
