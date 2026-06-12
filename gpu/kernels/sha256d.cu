// SHA-256d Bitcoin mining kernel.
//
// Each thread takes a nonce, finishes the double-SHA256 of the 80-byte block header, and checks
// the result against the target. The host precomputes the SHA-256 "midstate" of the first 64
// bytes of the header (which never change while the nonce varies), so the kernel only has to:
//   1. finish the FIRST SHA-256 by compressing the second header block (bytes 64..80 + padding),
//      resuming from `midstate`           -> H1
//   2. run a SECOND full SHA-256 over the 32-byte H1                  -> H2
//   3. interpret H2 as Bitcoin does and compare it against the target.
//
// Byte order (the classic Bitcoin footgun):
//   * The header is a little-endian byte stream, but SHA-256 packs bytes into BIG-endian 32-bit
//     words. The host passes the three "tail" words (last 4 bytes of the merkle root, the time,
//     and the bits) already packed big-endian exactly as SHA-256 sees them. The nonce is byte-
//     swapped here (it is stored little-endian in the header).
//   * Validity is `reverse(H2) < target`, i.e. H2's bytes read as a LITTLE-endian 256-bit number
//     must be below the target. `target` is passed as 8 big-endian 32-bit words, most-significant
//     word first (target[0] = most significant). We compare from the most significant end, which
//     for the number is byteswap(H2_word[7]) down to byteswap(H2_word[0]).

typedef unsigned int u32;

__device__ __constant__ u32 K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u,
};

__device__ __forceinline__ u32 rotr(u32 x, u32 n) { return (x >> n) | (x << (32 - n)); }
__device__ __forceinline__ u32 bswap(u32 x) { return __byte_perm(x, 0, 0x0123); }
__device__ __forceinline__ u32 Ch(u32 x, u32 y, u32 z)  { return (x & y) ^ (~x & z); }
__device__ __forceinline__ u32 Maj(u32 x, u32 y, u32 z) { return (x & y) ^ (x & z) ^ (y & z); }
__device__ __forceinline__ u32 bsig0(u32 x) { return rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22); }
__device__ __forceinline__ u32 bsig1(u32 x) { return rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25); }
__device__ __forceinline__ u32 ssig0(u32 x) { return rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3); }
__device__ __forceinline__ u32 ssig1(u32 x) { return rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10); }

// One SHA-256 block compression: expands the 16 input words and folds the result into `state`.
__device__ void sha256_transform(u32 state[8], const u32 in[16]) {
    u32 w[64];
#pragma unroll
    for (int i = 0; i < 16; i++) w[i] = in[i];
#pragma unroll
    for (int i = 16; i < 64; i++)
        w[i] = ssig1(w[i - 2]) + w[i - 7] + ssig0(w[i - 15]) + w[i - 16];

    u32 a = state[0], b = state[1], c = state[2], d = state[3];
    u32 e = state[4], f = state[5], g = state[6], h = state[7];

#pragma unroll
    for (int i = 0; i < 64; i++) {
        u32 t1 = h + bsig1(e) + Ch(e, f, g) + K[i] + w[i];
        u32 t2 = bsig0(a) + Maj(a, b, c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }

    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

extern "C" __global__ void mine(
    const u32 *__restrict__ midstate,   // 8 words: SHA-256 state after header bytes 0..64
    u32 tail0,                          // header bytes 64..68 (last 4 of merkle root), big-endian
    u32 tail1,                          // header bytes 68..72 (time), big-endian
    u32 tail2,                          // header bytes 72..76 (bits), big-endian
    const u32 *__restrict__ target,     // 8 big-endian words, target[0] most significant
    u32 base_nonce,
    u32 nonces_per_thread,
    u32 *__restrict__ out_count,        // atomic counter of hits
    u32 *__restrict__ out_nonces,       // hit nonces (capacity = out_capacity)
    u32 out_capacity)
{
    u32 idx = blockIdx.x * blockDim.x + threadIdx.x;
    u32 start = base_nonce + idx * nonces_per_thread;

    for (u32 j = 0; j < nonces_per_thread; j++) {
        u32 nonce = start + j;

        // ---- finish first SHA-256: second header block, resumed from midstate ----
        u32 s1[8];
#pragma unroll
        for (int i = 0; i < 8; i++) s1[i] = midstate[i];

        u32 w[16];
        w[0] = tail0;
        w[1] = tail1;
        w[2] = tail2;
        w[3] = bswap(nonce);     // nonce is little-endian in the header stream
        w[4] = 0x80000000u;      // padding: 0x80 byte after the 80-byte message
#pragma unroll
        for (int i = 5; i < 15; i++) w[i] = 0u;
        w[15] = 0x00000280u;     // message length in bits: 80 * 8 = 640
        sha256_transform(s1, w); // s1 = H1

        // ---- second SHA-256 over the 32-byte H1 ----
        u32 s2[8] = {
            0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
            0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
        };
        u32 w2[16];
#pragma unroll
        for (int i = 0; i < 8; i++) w2[i] = s1[i];
        w2[8] = 0x80000000u;     // padding after 32-byte message
#pragma unroll
        for (int i = 9; i < 15; i++) w2[i] = 0u;
        w2[15] = 0x00000100u;    // message length in bits: 32 * 8 = 256
        sha256_transform(s2, w2); // s2 = H2

        // ---- compare H2 (as a little-endian 256-bit number) against the target ----
        // Most significant chunk of the number is byteswap(s2[7]); least is byteswap(s2[0]).
        bool below = false;
#pragma unroll
        for (int k = 0; k < 8; k++) {
            u32 hw = bswap(s2[7 - k]);
            u32 tw = target[k];
            if (hw != tw) { below = (hw < tw); break; }
            // equal so far -> keep comparing lower words; if all equal, `below` stays false
            // (matches the host's strict `<` comparison).
        }

        if (below) {
            u32 pos = atomicAdd(out_count, 1u);
            if (pos < out_capacity) out_nonces[pos] = nonce;
        }
    }
}
