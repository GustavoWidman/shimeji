//! The registry of LSH implementations that shimeji is tested and measured
//! against.
//!
//! [`IMPLEMENTATIONS`] is the one list both the differential test and the
//! benchmark iterate; adding an implementation means adding an entry here.

mod kisa;

use shimeji::{Lsh256_224, Lsh256_256, Lsh512_224, Lsh512_256, Lsh512_384, Lsh512_512};

/// Bytes the build script recorded about how this crate and KISA's code were
/// compiled, one `key: value` per line.
pub const BUILD_INFO: &str = include_str!(concat!(env!("OUT_DIR"), "/build-info.txt"));

/// An LSH family: LSH-256 (32-bit words) or LSH-512 (64-bit words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Lsh256,
    Lsh512,
}

impl Family {
    /// Digest lengths in bytes the standard defines for this family.
    pub const fn digest_lengths(self) -> &'static [usize] {
        match self {
            Family::Lsh256 => &[28, 32],
            Family::Lsh512 => &[28, 32, 48, 64],
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Family::Lsh256 => "lsh256",
            Family::Lsh512 => "lsh512",
        }
    }
}

/// One of the six standardized hash functions, such as LSH-512-224.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Digest {
    pub family: Family,
    /// Digest length in bytes.
    pub len: usize,
}

impl Digest {
    pub fn all() -> impl Iterator<Item = Digest> {
        [Family::Lsh256, Family::Lsh512]
            .into_iter()
            .flat_map(|family| {
                family
                    .digest_lengths()
                    .iter()
                    .map(move |&len| Digest { family, len })
            })
    }

    /// The name used in result files, such as `lsh512-224`.
    pub fn name(self) -> String {
        format!("{}-{}", self.family.name(), 8 * self.len)
    }

    pub fn parse(name: &str) -> Option<Digest> {
        Digest::all().find(|digest| digest.name() == name)
    }
}

/// Hashes a message a given number of times into an output slice.
pub type Repeat = fn(&[u8], &mut [u8], u64);

/// One implementation of one LSH family.
pub struct Implementation {
    /// Name used in result files and on the command line, shared by an
    /// implementation's LSH-256 and LSH-512 entries.
    pub name: &'static str,
    pub family: Family,
    /// Digest lengths in bytes this implementation computes.
    pub digest_lengths: &'static [usize],
    /// Writes the digest of the message into the output slice, whose length
    /// selects the digest length (one of `digest_lengths`).
    pub hash: fn(&[u8], &mut [u8]),
    /// Hashes one message the given number of times in a loop inside the
    /// implementation's own language, leaving the digest in the output
    /// slice. The benchmark times this loop instead of calling
    /// `hash` once per message, for implementations behind a foreign call
    /// that costs as much as hashing a short message.
    pub repeat: Option<Repeat>,
    /// Whether the running CPU can execute this implementation.
    pub available: fn() -> bool,
}

impl Implementation {
    pub fn digests(&self) -> impl Iterator<Item = Digest> + '_ {
        self.digest_lengths.iter().map(|&len| Digest {
            family: self.family,
            len,
        })
    }
}

/// Every implementation compiled into this build, including ones the
/// running CPU cannot execute; [`implementations`] filters those out.
pub static IMPLEMENTATIONS: &[Implementation] = &[
    Implementation {
        name: "shimeji",
        family: Family::Lsh256,
        digest_lengths: Family::Lsh256.digest_lengths(),
        hash: shimeji_lsh256,
        repeat: None,
        available: always,
    },
    Implementation {
        name: "shimeji",
        family: Family::Lsh512,
        digest_lengths: Family::Lsh512.digest_lengths(),
        hash: shimeji_lsh512,
        repeat: None,
        available: always,
    },
    kisa::REF_256,
    kisa::REF_512,
    #[cfg(kisa_neon)]
    kisa::NEON_256,
    #[cfg(kisa_neon)]
    kisa::NEON_512,
    #[cfg(kisa_sse2)]
    kisa::SSE2_256,
    #[cfg(kisa_sse2)]
    kisa::SSE2_512,
    #[cfg(kisa_ssse3)]
    kisa::SSSE3_256,
    #[cfg(kisa_ssse3)]
    kisa::SSSE3_512,
    #[cfg(kisa_avx2)]
    kisa::AVX2_256,
    #[cfg(kisa_avx2)]
    kisa::AVX2_512,
];

/// The implementations the running CPU can execute.
pub fn implementations() -> impl Iterator<Item = &'static Implementation> {
    IMPLEMENTATIONS.iter().filter(|imp| (imp.available)())
}

fn always() -> bool {
    true
}

fn shimeji_lsh256(message: &[u8], out: &mut [u8]) {
    match out.len() {
        28 => out.copy_from_slice(&Lsh256_224::digest(message)),
        32 => out.copy_from_slice(&Lsh256_256::digest(message)),
        len => panic!("LSH-256 has no {len}-byte digest"),
    }
}

fn shimeji_lsh512(message: &[u8], out: &mut [u8]) {
    match out.len() {
        28 => out.copy_from_slice(&Lsh512_224::digest(message)),
        32 => out.copy_from_slice(&Lsh512_256::digest(message)),
        48 => out.copy_from_slice(&Lsh512_384::digest(message)),
        64 => out.copy_from_slice(&Lsh512_512::digest(message)),
        len => panic!("LSH-512 has no {len}-byte digest"),
    }
}

/// SplitMix64: the deterministic generator behind test messages, benchmark
/// inputs and bootstrap resampling.
pub struct Rng(u64);

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A uniform index below `n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub fn fill(&mut self, buf: &mut [u8]) {
        for chunk in buf.chunks_mut(8) {
            chunk.copy_from_slice(&self.next_u64().to_le_bytes()[..chunk.len()]);
        }
    }
}
