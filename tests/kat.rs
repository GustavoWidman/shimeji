use shimeji::{Lsh256_224, Lsh256_256, Lsh512_256, Lsh512_384, Lsh512_512, lsh256, lsh512};

mod common;

use common::SplitMix64;

struct Vector {
    name: String,
    message: Vec<u8>,
    digest: Vec<u8>,
}

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Parses a message field: hex, optionally split by spaces, where a token
/// `r<count>` repeats the token after it `count` times.
fn message(field: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut repeat = 1;
    for token in field.split_whitespace() {
        match token.strip_prefix('r') {
            Some(count) => repeat = count.parse().unwrap(),
            None => {
                out.extend(hex(token).repeat(repeat));
                repeat = 1;
            }
        }
    }
    out
}

fn vectors(file: &str) -> Vec<Vector> {
    let path = format!("{}/refs/kat/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).unwrap();
    let mut name = String::new();
    let mut pending = None;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Name:") {
            name = v.trim().to_owned();
        } else if let Some(v) = line.strip_prefix("Message:") {
            pending = Some(message(v));
        } else if let Some(v) = line.strip_prefix("Digest:") {
            out.push(Vector {
                name: name.clone(),
                message: pending.take().unwrap(),
                digest: hex(v),
            });
        }
    }
    out
}

/// The paths that hash one digest: `digest` and the streaming hasher fed one
/// piece per `update` call.
struct Paths {
    block: usize,
    digest: fn(&[u8]) -> Vec<u8>,
    stream: fn(&[&[u8]]) -> Vec<u8>,
}

macro_rules! paths {
    ($hasher:ty, $family:ident) => {
        Paths {
            block: $family::BLOCK,
            digest: |message| <$hasher>::digest(message).to_vec(),
            stream: |pieces| {
                let mut hasher = <$hasher>::new();
                for piece in pieces {
                    hasher.update(piece);
                }
                hasher.finalize().to_vec()
            },
        }
    };
}

fn paths(name: &str) -> Paths {
    match name {
        "LSH-224" => paths!(Lsh256_224, lsh256),
        "LSH-256" => paths!(Lsh256_256, lsh256),
        "LSH-384" => paths!(Lsh512_384, lsh512),
        "LSH-512" => paths!(Lsh512_512, lsh512),
        "LSH-512-256" => paths!(Lsh512_256, lsh512),
        other => panic!("no hasher for {other}"),
    }
}

/// Split sequences per vector for the streaming hasher.
const SPLITS: usize = 8;

/// The seed of the split sequences: `KAT_SEED` (decimal or `0x` hex) if set.
fn seed() -> u64 {
    match std::env::var("KAT_SEED") {
        Ok(v) => match v.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16).unwrap(),
            None => v.parse().unwrap(),
        },
        Err(_) => 0x6b61_745f_7370_6c74,
    }
}

/// Offsets that cut a `len`-byte message into the pieces of one streaming
/// pass: ascending, from 0 to `len`, so each pair of neighbours bounds one
/// `update`. A repeated offset is an empty update. About half of the cuts
/// land within two bytes of a multiple of `block`.
fn cuts(rng: &mut SplitMix64, len: usize, block: usize) -> Vec<usize> {
    let mut cuts = vec![0, len];
    for _ in 0..rng.below(12) {
        let cut = match rng.below(4) {
            0 => rng.below(len + 1),
            1 => cuts[rng.below(cuts.len())],
            _ => (rng.below(len / block + 2) * block + rng.below(5))
                .saturating_sub(2)
                .min(len),
        };
        cuts.push(cut);
    }
    cuts.sort_unstable();
    cuts
}

fn check(file: &str, expected: usize) {
    let vectors = vectors(file);
    assert_eq!(vectors.len(), expected, "{file}: vector count");
    let seed = seed();
    let mut rng = SplitMix64(seed);
    for v in &vectors {
        let paths = paths(&v.name);
        let len = v.message.len();
        assert_eq!(
            (paths.digest)(&v.message),
            v.digest,
            "{} on {len} bytes",
            v.name
        );
        assert_eq!(
            (paths.stream)(&[&v.message]),
            v.digest,
            "{} on {len} bytes in one update",
            v.name
        );
        for _ in 0..SPLITS {
            let cuts = cuts(&mut rng, len, paths.block);
            let pieces: Vec<&[u8]> = cuts.windows(2).map(|w| &v.message[w[0]..w[1]]).collect();
            assert_eq!(
                (paths.stream)(&pieces),
                v.digest,
                "{} on {len} bytes in updates cut at {cuts:?} (KAT_SEED={seed:#x})",
                v.name
            );
        }
    }
}

#[test]
fn lsh256() {
    check("lsh256.txt", 294);
}

#[test]
fn lsh512() {
    check("lsh512.txt", 294);
}

#[test]
fn lsh512_256() {
    check("lsh512_256.txt", 147);
}
