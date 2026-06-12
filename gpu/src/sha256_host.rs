//! Host-side SHA-256 block compression, used to precompute the **midstate** of the first
//! 64 bytes of the 80-byte block header (which never change while the nonce varies).
//!
//! The GPU kernel resumes from this midstate and only processes the second header block plus
//! the second SHA-256, instead of redoing the whole 80-byte first hash for every nonce.
//! That removes ~1/3 of the SHA-256 work per nonce.
//!
//! `midstate` returns the 8 state words **after** compressing exactly one 64-byte block,
//! starting from the SHA-256 initial vector. These words are uploaded to the GPU as-is.

const IV: [u32; 8] = [
    0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a,
    0x510e_527f, 0x9b05_688c, 0x1f83_d9ab, 0x5be0_cd19,
];

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[inline]
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[4 * i],
            block[4 * i + 1],
            block[4 * i + 2],
            block[4 * i + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// SHA-256 state after compressing the first 64 bytes of the header, starting from the IV.
/// These 8 words are uploaded to the GPU; the kernel resumes from here.
pub fn midstate(first64: &[u8; 64]) -> [u32; 8] {
    let mut state = IV;
    compress(&mut state, first64);
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// Full SHA-256 built from our block compression + padding, to validate `compress`
    /// against the reference `sha2` implementation.
    fn sha256_full(msg: &[u8]) -> [u8; 32] {
        let mut state = IV;
        let mut padded = msg.to_vec();
        padded.push(0x80);
        while padded.len() % 64 != 56 {
            padded.push(0);
        }
        let bits = (msg.len() as u64) * 8;
        padded.extend_from_slice(&bits.to_be_bytes());
        for chunk in padded.chunks_exact(64) {
            let mut block = [0u8; 64];
            block.copy_from_slice(chunk);
            compress(&mut state, &block);
        }
        let mut out = [0u8; 32];
        for (i, word) in state.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    #[test]
    fn matches_reference_sha256() {
        for msg in [&b""[..], &b"abc"[..], &b"The quick brown fox jumps over the lazy dog"[..]] {
            let ours = sha256_full(msg);
            let theirs: [u8; 32] = Sha256::digest(msg).into();
            assert_eq!(ours, theirs, "mismatch for {:?}", msg);
        }
    }

    #[test]
    fn midstate_is_first_block_state() {
        // A 64-byte block compressed via `midstate` must equal the state our full
        // implementation reaches after the same first block.
        let block: [u8; 64] = std::array::from_fn(|i| i as u8);
        let mut expected = IV;
        compress(&mut expected, &block);
        assert_eq!(midstate(&block), expected);
    }
}
