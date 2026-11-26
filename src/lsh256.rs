//! LSH-256: 32-bit words, 128-byte blocks, 26 steps.

crate::lsh::lsh! {
    /// LSH-256 with an `N`-byte digest: 28 (LSH-256-224) or 32 (LSH-256-256).
    ///
    /// ```
    /// assert_eq!(shimeji::Lsh256::<28>::digest(b"abc").len(), 28);
    /// ```
    ///
    /// Any other length is a compile-time error:
    ///
    /// ```compile_fail,E0080
    /// let digest = shimeji::Lsh256::<33>::digest(b"abc");
    /// ```
    hasher: Lsh256,
    word: u32,
    steps: 26,
    pairs: [1 2 3 4 5 6 7 8 9 10 11 12],
    rotations: { even: (29, 1), odd: (5, 17) },
    gamma: [0, 8, 16, 24, 24, 16, 8, 0],
    sc0: [
        0x917caf90, 0x6c1b10a2, 0x6f352943, 0xcf778243, 0x2ceb7472, 0x29e96ff2, 0x8a9ba428, 0x2eeb2642,
    ],
    ivs: {
        28 => [
            0x068608d3, 0x62d8f7a7, 0xd76652ab, 0x4c600a43, 0xbdc40aa8, 0x1eca0b68, 0xda1a89be, 0x3147d354,
            0x707eb4f9, 0xf65b3862, 0x6b0b2abe, 0x56b8ec0a, 0xcf237286, 0xee0d1727, 0x33636595, 0x8bb8d05f,
        ],
        32 => [
            0x46a10f1f, 0xfddce486, 0xb41443a8, 0x198e6b9d, 0x3304388d, 0xb0f5a3c7, 0xb36061c4, 0x7adbd553,
            0x105d5378, 0x2f74de54, 0x5c2f2d95, 0xf2553fbe, 0x8051357a, 0x138668c8, 0x47aa4484, 0xe01afb41,
        ],
    },
    neon_message: neon::Message,
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
mod neon;
#[cfg(target_arch = "x86_64")]
mod x86;
