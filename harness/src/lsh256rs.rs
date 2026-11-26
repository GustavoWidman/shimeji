//! The `lsh256` crate
//! (https://github.com/corp0ratestarts-cmd/LSH-256-Korea-KS-X-3262-RUST),
//! portable Rust without `unsafe`, a git dependency of the harness pinned by
//! commit. Its one-shot functions return arrays, but each finalizes into a
//! `Vec`, so every message costs one heap allocation.

use lsh256::{Lsh256, Lsh512};

use crate::{Family, Implementation};

fn lsh256(message: &[u8], out: &mut [u8]) {
    match out.len() {
        28 => out.copy_from_slice(&Lsh256::hash_224(message)),
        32 => out.copy_from_slice(&Lsh256::hash_256(message)),
        len => panic!("LSH-256 has no {len}-byte digest"),
    }
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    match out.len() {
        28 => out.copy_from_slice(&Lsh512::hash_512_224(message)),
        32 => out.copy_from_slice(&Lsh512::hash_512_256(message)),
        48 => out.copy_from_slice(&Lsh512::hash_384(message)),
        64 => out.copy_from_slice(&Lsh512::hash_512(message)),
        len => panic!("LSH-512 has no {len}-byte digest"),
    }
}

pub(crate) const LSH256: Implementation = Implementation {
    name: "lsh256-rs",
    family: Family::Lsh256,
    digest_lengths: Family::Lsh256.digest_lengths(),
    hash: lsh256,
    repeat: None,
    available: || true,
};

pub(crate) const LSH512: Implementation = Implementation {
    name: "lsh256-rs",
    family: Family::Lsh512,
    digest_lengths: Family::Lsh512.digest_lengths(),
    hash: lsh512,
    repeat: None,
    available: || true,
};
