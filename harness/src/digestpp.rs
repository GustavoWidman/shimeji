//! digestpp's LSH (https://github.com/kerukuro/digestpp), portable
//! header-only C++, called through the exports in `c/digestpp.cpp`, which
//! `build.rs` compiles against the commit `flake.nix` pins.
//!
//! Each message gets a new hasher, which derives its IV with one compression
//! function call on a zero block, as digestpp does for every digest length.
//! Hashing in a C++ loop instead of one call per message changes its time
//! for 64-byte messages by less than 0.5%, so it has no `repeat`.

use crate::{Family, Implementation};

/// Hashes the message and writes the `outlen`-byte digest; returns -1 for a
/// digest length it lacks.
type Export = unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32;

unsafe extern "C" {
    fn digestpp_lsh256(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
    fn digestpp_lsh512(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
}

fn call(export: Export, message: &[u8], out: &mut [u8]) {
    // SAFETY: digestpp reads `message.len()` bytes and writes `out.len()`.
    let err = unsafe { export(message.as_ptr(), message.len(), out.as_mut_ptr(), out.len()) };
    assert_eq!(err, 0, "digestpp has no {}-byte digest", out.len());
}

fn lsh256(message: &[u8], out: &mut [u8]) {
    call(digestpp_lsh256, message, out)
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    call(digestpp_lsh512, message, out)
}

pub(crate) const LSH256: Implementation = Implementation {
    name: "digestpp",
    family: Family::Lsh256,
    digest_lengths: Family::Lsh256.digest_lengths(),
    hash: lsh256,
    repeat: None,
    available: || true,
};

pub(crate) const LSH512: Implementation = Implementation {
    name: "digestpp",
    family: Family::Lsh512,
    digest_lengths: Family::Lsh512.digest_lengths(),
    hash: lsh512,
    repeat: None,
    available: || true,
};
