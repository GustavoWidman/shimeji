//! Crypto++'s LSH (https://github.com/weidai11/cryptopp), C++ that picks
//! portable, SSSE3 or AVX2 code from the CPU at run time. `build.rs` builds
//! the library from the commit `flake.nix` pins and links it with the shim in
//! `c/cryptopp.cpp`.
//!
//! Each entry is named after one of Crypto++'s paths and is available exactly
//! when Crypto++ reports, through `AlgorithmProvider`, that it takes it.
//! Crypto++ has no LSH-512-224 class; the shim derives one from its
//! `LSH512_Base`.

use std::ffi::{CStr, c_char};

use crate::{Family, Implementation};

/// Hashes the message and writes the `outlen`-byte digest; returns -1 for a
/// digest length it lacks.
type Export = unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32;

unsafe extern "C" {
    fn cryptopp_lsh256(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
    fn cryptopp_lsh512(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
    fn cryptopp_lsh256_path() -> *const c_char;
    fn cryptopp_lsh512_path() -> *const c_char;
}

fn call(export: Export, message: &[u8], out: &mut [u8]) {
    // SAFETY: Crypto++ reads `message.len()` bytes and writes `out.len()`.
    let err = unsafe { export(message.as_ptr(), message.len(), out.as_mut_ptr(), out.len()) };
    assert_eq!(err, 0, "Crypto++ has no {}-byte digest", out.len());
}

fn lsh256(message: &[u8], out: &mut [u8]) {
    call(cryptopp_lsh256, message, out)
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    call(cryptopp_lsh512, message, out)
}

/// The path Crypto++ takes for `family` on this CPU.
fn path(family: Family) -> &'static str {
    // SAFETY: both return a static NUL-terminated string.
    let path = unsafe {
        CStr::from_ptr(match family {
            Family::Lsh256 => cryptopp_lsh256_path(),
            Family::Lsh512 => cryptopp_lsh512_path(),
        })
    };
    path.to_str().unwrap()
}

macro_rules! cryptopp {
    ($entry:ident, $name:literal, $family:ident, $hash:ident, $path:literal) => {
        pub(crate) const $entry: Implementation = Implementation {
            name: $name,
            family: Family::$family,
            digest_lengths: Family::$family.digest_lengths(),
            hash: $hash,
            repeat: None,
            available: || path(Family::$family) == $path,
        };
    };
}

cryptopp!(REF_256, "cryptopp-ref", Lsh256, lsh256, "ref");
cryptopp!(REF_512, "cryptopp-ref", Lsh512, lsh512, "ref");
cryptopp!(SSSE3_256, "cryptopp-ssse3", Lsh256, lsh256, "ssse3");
cryptopp!(SSSE3_512, "cryptopp-ssse3", Lsh512, lsh512, "ssse3");
cryptopp!(AVX2_256, "cryptopp-avx2", Lsh256, lsh256, "avx2");
cryptopp!(AVX2_512, "cryptopp-avx2", Lsh512, lsh512, "avx2");
