//! go-krypto picks its assembly from the CPU's features when the Go runtime
//! starts (`lsh256/asm_*.go`, `lsh512/asm_*.go`). Each entry is named after
//! one of those paths and is available exactly when go-krypto takes it. On
//! x86-64, LSH-256 never takes its AVX2 code, which go-krypto disables as
//! slower than its SSSE3 code.
//!
//! A call into Go costs tens of nanoseconds, so every entry has `repeat`, and
//! the benchmark times go-krypto's own loop.

use crate::{Family, Implementation};

type Export = unsafe extern "C" fn(*const u8, usize, *mut u8, usize, u64) -> i32;

unsafe extern "C" {
    fn gokrypto_lsh256(
        msg: *const u8,
        msglen: usize,
        out: *mut u8,
        outlen: usize,
        iters: u64,
    ) -> i32;
    fn gokrypto_lsh512(
        msg: *const u8,
        msglen: usize,
        out: *mut u8,
        outlen: usize,
        iters: u64,
    ) -> i32;
}

fn call(export: Export, message: &[u8], out: &mut [u8], iters: u64) {
    // SAFETY: Go reads `message.len()` bytes and writes `out.len()` bytes.
    let err = unsafe {
        export(
            message.as_ptr(),
            message.len(),
            out.as_mut_ptr(),
            out.len(),
            iters,
        )
    };
    assert_eq!(err, 0, "go-krypto has no {}-byte digest", out.len());
}

fn lsh256(message: &[u8], out: &mut [u8]) {
    call(gokrypto_lsh256, message, out, 1)
}

fn lsh256_repeat(message: &[u8], out: &mut [u8], iters: u64) {
    call(gokrypto_lsh256, message, out, iters)
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    call(gokrypto_lsh512, message, out, 1)
}

fn lsh512_repeat(message: &[u8], out: &mut [u8], iters: u64) {
    call(gokrypto_lsh512, message, out, iters)
}

macro_rules! gokrypto {
    ($entry:ident, $name:literal, $family:ident, $hash:ident, $repeat:ident, $available:expr) => {
        pub(crate) const $entry: Implementation = Implementation {
            name: $name,
            family: Family::$family,
            digest_lengths: Family::$family.digest_lengths(),
            hash: $hash,
            repeat: Some($repeat),
            available: $available,
        };
    };
}

#[cfg(target_arch = "aarch64")]
gokrypto!(
    NEON_256,
    "go-krypto-neon",
    Lsh256,
    lsh256,
    lsh256_repeat,
    || true
);
#[cfg(target_arch = "aarch64")]
gokrypto!(
    NEON_512,
    "go-krypto-neon",
    Lsh512,
    lsh512,
    lsh512_repeat,
    || true
);

#[cfg(target_arch = "x86_64")]
mod x86 {
    pub(super) fn ssse3() -> bool {
        is_x86_feature_detected!("ssse3")
    }

    /// go-krypto's `hasAVX2`: SSSE3, AVX and AVX2.
    pub(super) fn avx2() -> bool {
        ssse3() && is_x86_feature_detected!("avx") && is_x86_feature_detected!("avx2")
    }
}

#[cfg(target_arch = "x86_64")]
gokrypto!(
    SSE2_256,
    "go-krypto-sse2",
    Lsh256,
    lsh256,
    lsh256_repeat,
    || !x86::ssse3()
);
#[cfg(target_arch = "x86_64")]
gokrypto!(
    SSSE3_256,
    "go-krypto-ssse3",
    Lsh256,
    lsh256,
    lsh256_repeat,
    x86::ssse3
);
#[cfg(target_arch = "x86_64")]
gokrypto!(
    SSE2_512,
    "go-krypto-sse2",
    Lsh512,
    lsh512,
    lsh512_repeat,
    || !x86::ssse3()
);
#[cfg(target_arch = "x86_64")]
gokrypto!(
    SSSE3_512,
    "go-krypto-ssse3",
    Lsh512,
    lsh512,
    lsh512_repeat,
    || { x86::ssse3() && !x86::avx2() }
);
#[cfg(target_arch = "x86_64")]
gokrypto!(
    AVX2_512,
    "go-krypto-avx2",
    Lsh512,
    lsh512,
    lsh512_repeat,
    x86::avx2
);
