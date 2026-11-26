use crate::{Family, Implementation};

type Repeat = unsafe extern "C" fn(i32, *const u8, u64, *mut u8, u64) -> i32;

fn call(repeat: Repeat, message: &[u8], out: &mut [u8], iters: u64) {
    // SAFETY: the code reads `8 * message.len()` bits from `message` and
    // writes `out.len()` bytes, the digest length it is given in bits.
    let err = unsafe {
        repeat(
            8 * out.len() as i32,
            message.as_ptr(),
            8 * message.len() as u64,
            out.as_mut_ptr(),
            iters,
        )
    };
    assert_eq!(err, 0, "LSH-SIMD rejected a {}-byte digest", out.len());
}

macro_rules! lsh_simd {
    ($entry:ident, $name:literal, $family:ident, $symbol:ident, $feature:tt) => {
        pub(crate) const $entry: Implementation = {
            unsafe extern "C" {
                fn $symbol(
                    hashbitlen: i32,
                    data: *const u8,
                    databitlen: u64,
                    hashval: *mut u8,
                    iters: u64,
                ) -> i32;
            }
            fn hash(message: &[u8], out: &mut [u8]) {
                call($symbol, message, out, 1)
            }
            fn repeat(message: &[u8], out: &mut [u8], iters: u64) {
                call($symbol, message, out, iters)
            }
            Implementation {
                name: $name,
                family: Family::$family,
                digest_lengths: Family::$family.digest_lengths(),
                hash,
                repeat: Some(repeat),
                available: || is_x86_feature_detected!($feature),
            }
        };
    };
}

lsh_simd!(
    SSE2_256,
    "lsh-simd-sse2",
    Lsh256,
    lsh_simd_sse2_lsh256_repeat,
    "sse2"
);
lsh_simd!(
    SSE2_512,
    "lsh-simd-sse2",
    Lsh512,
    lsh_simd_sse2_lsh512_repeat,
    "sse2"
);
lsh_simd!(
    SSSE3_256,
    "lsh-simd-ssse3",
    Lsh256,
    lsh_simd_ssse3_lsh256_repeat,
    "ssse3"
);
lsh_simd!(
    SSSE3_512,
    "lsh-simd-ssse3",
    Lsh512,
    lsh_simd_ssse3_lsh512_repeat,
    "ssse3"
);
lsh_simd!(
    AVX2_256,
    "lsh-simd-avx2",
    Lsh256,
    lsh_simd_avx2_lsh256_repeat,
    "avx2"
);
lsh_simd!(
    AVX2_512,
    "lsh-simd-avx2",
    Lsh512,
    lsh_simd_avx2_lsh512_repeat,
    "avx2"
);
