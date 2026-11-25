//! LSH-256 and LSH-512, the hash functions of the Korean standard KS X 3262.
//!
//! ```
//! let digest = shimeji::Lsh256_256::digest(b"abc");
//! assert_eq!(digest.len(), 32);
//! ```
#![no_std]
#![allow(non_camel_case_types)]

mod lsh;
pub mod lsh256;
pub mod lsh512;
#[cfg(target_arch = "x86_64")]
mod x86;

pub use lsh256::Lsh256;
pub use lsh512::Lsh512;

/// LSH-256-224.
pub type Lsh256_224 = Lsh256<28>;
/// LSH-256-256.
pub type Lsh256_256 = Lsh256<32>;
/// LSH-512-224.
pub type Lsh512_224 = Lsh512<28>;
/// LSH-512-256.
pub type Lsh512_256 = Lsh512<32>;
/// LSH-512-384.
pub type Lsh512_384 = Lsh512<48>;
/// LSH-512-512.
pub type Lsh512_512 = Lsh512<64>;
