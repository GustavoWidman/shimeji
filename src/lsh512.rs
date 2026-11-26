//! LSH-512: 64-bit words, 256-byte blocks, 28 steps.

crate::lsh::lsh! {
    /// LSH-512 with an `N`-byte digest: 28, 32, 48 or 64 (LSH-512-224, -256, -384, -512).
    ///
    /// ```
    /// assert_eq!(shimeji::Lsh512::<48>::digest(b"abc").len(), 48);
    /// ```
    ///
    /// Any other length is a compile-time error:
    ///
    /// ```compile_fail,E0080
    /// let digest = shimeji::Lsh512::<33>::digest(b"abc");
    /// ```
    hasher: Lsh512,
    word: u64,
    steps: 28,
    pairs: [1 2 3 4 5 6 7 8 9 10 11 12 13],
    rotations: { even: (23, 59), odd: (7, 3) },
    gamma: [0, 16, 32, 48, 8, 24, 40, 56],
    sc0: [
        0x97884283c938982a, 0xba1fca93533e2355, 0xc519a2e87aeb1c03, 0x9a0fc95462af17b1,
        0xfc3dda8ab019a82b, 0x02825d079a895407, 0x79f2d0a7ee06a6f7, 0xd76d15eed9fdf5fe,
    ],
    ivs: {
        28 => [
            0x0c401e9fe8813a55, 0x4a5f446268fd3d35, 0xff13e452334f612a, 0xf8227661037e354a,
            0xa5f223723c9ca29d, 0x95d965a11aed3979, 0x01e23835b9ab02cc, 0x52d49cbad5b30616,
            0x9e5c2027773f4ed3, 0x66a5c8801925b701, 0x22bbc85b4c6779d9, 0xc13171a42c559c23,
            0x31e2b67d25be3813, 0xd522c4deed8e4d83, 0xa79f5509b43fbafe, 0xe00d2cd88b4b6c6a,
        ],
        32 => [
            0x6dc57c33df989423, 0xd8ea7f6e8342c199, 0x76df8356f8603ac4, 0x40f1b44de838223a,
            0x39ffe7cfc31484cd, 0x39c4326cc5281548, 0x8a2ff85a346045d8, 0xff202aa46dbdd61e,
            0xcf785b3cd5fcdb8b, 0x1f0323b64a8150bf, 0xff75d972f29ea355, 0x2e567f30bf1ca9e1,
            0xb596875bf8ff6dba, 0xfcca39b089ef4615, 0xecff4017d020b4b6, 0x7e77384c772ed802,
        ],
        48 => [
            0x53156a66292808f6, 0xb2c4f362b204c2bc, 0xb84b7213bfa05c4e, 0x976ceb7c1b299f73,
            0xdf0cc63c0570ae97, 0xda4441baa486ce3f, 0x6559f5d9b5f2acc2, 0x22dacf19b4b52a16,
            0xbbcdacefde80953a, 0xc9891a2879725b3e, 0x7c9fe6330237e440, 0xa30ba550553f7431,
            0xbb08043fb34e3e30, 0xa0dec48d54618ead, 0x150317267464bc57, 0x32d1501fde63dc93,
        ],
        64 => [
            0xadd50f3c7f07094e, 0xe3f3cee8f9418a4f, 0xb527ecde5b3d0ae9, 0x2ef6dec68076f501,
            0x8cb994cae5aca216, 0xfbb9eae4bba48cc7, 0x650a526174725fea, 0x1f9a61a73f8d8085,
            0xb6607378173b539b, 0x1bc99853b0c0b9ed, 0xdf727fc19b182d47, 0xdbef360cf893a457,
            0x4981f5e570147e80, 0xd00c4490ca7d3e30, 0x5d73940c0e4ae1ec, 0x894085e2edb2d819,
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
