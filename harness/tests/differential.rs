//! Differential test: every implementation in the registry must produce the
//! same digests as shimeji's portable code, for all six digests.
//!
//! The messages are every length from 0 to 3000 bytes, which covers every
//! length within two bytes of a multiple of the 128-byte LSH-256 block and the
//! 256-byte LSH-512 block up to four blocks, plus lengths around 64 KiB and
//! 1 MiB. Contents come from a fixed seed, and each message starts at a
//! varying offset in its buffer so unaligned loads are exercised.
//!
//! LSH-512-224 has no Crypto++ test vector, so for it the comparisons with
//! the other implementations are the only check.

use harness::{Digest, Family, Implementation, Rng, implementations};

const LONG: [usize; 4] = [65_535, 65_536, 65_537, 1 << 20];

fn lengths() -> impl Iterator<Item = usize> {
    (0..=3000).chain(LONG)
}

/// shimeji's portable code, which every other entry is compared with.
fn portable(family: Family) -> &'static Implementation {
    implementations()
        .find(|imp| imp.name == "shimeji-portable" && imp.family == family)
        .unwrap()
}

#[test]
fn every_digest_has_an_independent_implementation() {
    for digest in Digest::all() {
        assert!(
            implementations()
                .any(|imp| !imp.name.starts_with("shimeji") && imp.digests().any(|d| d == digest)),
            "nothing to compare {} against",
            digest.name()
        );
    }
}

#[test]
fn every_implementation_matches_portable() {
    let mut rng = Rng::new(0x5348_494d_454a_4931);
    let mut buf = vec![0u8; (1 << 20) + 64];
    let mut mismatches = Vec::new();
    let mut compared = 0usize;
    for len in lengths() {
        let offset = rng.below(64);
        let message = &mut buf[offset..offset + len];
        rng.fill(message);
        let message = &*message;
        for imp in implementations().filter(|imp| imp.name != "shimeji-portable") {
            let reference = portable(imp.family);
            for digest in imp.digests() {
                let mut want = [0u8; 64];
                let mut got = [0u8; 64];
                (reference.hash)(message, &mut want[..digest.len]);
                (imp.hash)(message, &mut got[..digest.len]);
                compared += 1;
                if want != got {
                    mismatches.push(format!("{} {} length {len}", imp.name, digest.name()));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {compared} digests differ, first: {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(10)]
    );
    eprintln!("{compared} digests compared");
}
