//! KISA's C implementations, compiled by `build.rs` under the symbol prefix
//! `kisa_<variant>_`.

use crate::{Family, Implementation};

/// Bit 16 of KISA's `lsh_type` selects LSH-512; the low 16 bits hold the
/// digest length in bytes (`LSH_MAKE_TYPE` in `lsh_def.h`).
fn algtype(family: Family, len: usize) -> u32 {
    let family = match family {
        Family::Lsh256 => 0,
        Family::Lsh512 => 0x10000,
    };
    family | len as u32
}

/// A 64-byte buffer, aligned as the SIMD variants may require of the digest
/// output (`lsh.h`: "hashval ... alignment가 맞아야한다").
#[repr(C, align(32))]
struct Out([u8; 64]);

/// Calls a KISA `lsh*_repeat`, which runs `lsh*_digest` `iters` times, and
/// copies the digest into `out`.
fn call(
    repeat: unsafe extern "C" fn(u32, *const u8, usize, *mut u8, u64) -> u32,
    family: Family,
    message: &[u8],
    out: &mut [u8],
    iters: u64,
) {
    let mut buf = Out([0; 64]);
    // SAFETY: KISA reads `8 * message.len()` bits from `message` and writes at
    // most 64 bytes to the buffer.
    let err = unsafe {
        repeat(
            algtype(family, out.len()),
            message.as_ptr(),
            8 * message.len(),
            buf.0.as_mut_ptr(),
            iters,
        )
    };
    assert_eq!(err, 0, "KISA lsh_err {err:#x}");
    out.copy_from_slice(&buf.0[..out.len()]);
}

macro_rules! kisa {
    ($entry:ident, $name:literal, $family:ident, $symbol:ident, $available:expr) => {
        pub(crate) const $entry: Implementation = {
            unsafe extern "C" {
                fn $symbol(
                    algtype: u32,
                    data: *const u8,
                    databitlen: usize,
                    hashval: *mut u8,
                    iters: u64,
                ) -> u32;
            }
            fn hash(message: &[u8], out: &mut [u8]) {
                call($symbol, Family::$family, message, out, 1)
            }
            fn repeat(message: &[u8], out: &mut [u8], iters: u64) {
                call($symbol, Family::$family, message, out, iters)
            }
            Implementation {
                name: $name,
                family: Family::$family,
                digest_lengths: Family::$family.digest_lengths(),
                hash,
                repeat: Some(repeat),
                available: $available,
            }
        };
    };
}

fn always() -> bool {
    true
}

kisa!(REF_256, "kisa-ref", Lsh256, kisa_ref_lsh256_repeat, always);
kisa!(REF_512, "kisa-ref", Lsh512, kisa_ref_lsh512_repeat, always);

#[cfg(kisa_neon)]
kisa!(
    NEON_256,
    "kisa-neon",
    Lsh256,
    kisa_neon_lsh256_repeat,
    always
);
#[cfg(kisa_neon)]
kisa!(
    NEON_512,
    "kisa-neon",
    Lsh512,
    kisa_neon_lsh512_repeat,
    always
);

#[cfg(kisa_sse2)]
kisa!(
    SSE2_256,
    "kisa-sse2",
    Lsh256,
    kisa_sse2_lsh256_repeat,
    || is_x86_feature_detected!("sse2")
);
#[cfg(kisa_sse2)]
kisa!(
    SSE2_512,
    "kisa-sse2",
    Lsh512,
    kisa_sse2_lsh512_repeat,
    || is_x86_feature_detected!("sse2")
);

#[cfg(kisa_ssse3)]
kisa!(
    SSSE3_256,
    "kisa-ssse3",
    Lsh256,
    kisa_ssse3_lsh256_repeat,
    || is_x86_feature_detected!("ssse3")
);
#[cfg(kisa_ssse3)]
kisa!(
    SSSE3_512,
    "kisa-ssse3",
    Lsh512,
    kisa_ssse3_lsh512_repeat,
    || is_x86_feature_detected!("ssse3")
);

#[cfg(kisa_avx2)]
kisa!(
    AVX2_256,
    "kisa-avx2",
    Lsh256,
    kisa_avx2_lsh256_repeat,
    || is_x86_feature_detected!("avx2")
);
#[cfg(kisa_avx2)]
kisa!(
    AVX2_512,
    "kisa-avx2",
    Lsh512,
    kisa_avx2_lsh512_repeat,
    || is_x86_feature_detected!("avx2")
);
