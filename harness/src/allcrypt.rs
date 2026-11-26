use allcrypt::hash_functions::HashFunction;
use allcrypt::hash_functions::lsh::{Lsh256, Lsh512};

use crate::{Family, Implementation};

fn lsh256(message: &[u8], out: &mut [u8]) {
    let mut hasher = Lsh256::new(out.len()).unwrap();
    hasher.update(message);
    out.copy_from_slice(&hasher.digest());
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    let mut hasher = Lsh512::new(out.len()).unwrap();
    hasher.update(message);
    out.copy_from_slice(&hasher.digest());
}

pub(crate) const LSH256: Implementation = Implementation {
    name: "allcrypt",
    family: Family::Lsh256,
    digest_lengths: Family::Lsh256.digest_lengths(),
    hash: lsh256,
    repeat: None,
    available: || true,
};

pub(crate) const LSH512: Implementation = Implementation {
    name: "allcrypt",
    family: Family::Lsh512,
    digest_lengths: Family::Lsh512.digest_lengths(),
    hash: lsh512,
    repeat: None,
    available: || true,
};
