# shimeji

An implementation of LSH-256 and LSH-512, the hash functions of the Korean
standard KS X 3262, in Rust.

Gustavo Wagon Widman and João Victor Wandermurem, Computer Engineering,
Instituto de Tecnologia e Liderança (Inteli), São Paulo, Brazil.

The paper is [PAPER.pdf](PAPER.pdf). Its typst sources are in `paper/`.

## Overview

LSH is a family of wide-pipe Merkle–Damgård hash functions. Korean
cryptography uses it where other standards use SHA-2, in PBKDF2 and in the
hash-based signature schemes K-XMSS, K-SPHINCS+ and K-LMS. Its optimized
implementations are written in C (KISA's reference code), C++ (Crypto++) and
Go (go-krypto). The Rust implementations, among them the allcrypt and lsh256
crates, are portable code without SIMD.

shimeji implements both functions and all six digest lengths in Rust. The
paper describes the implementation and measures it against the C, C++ and Go
code on an Apple M6 and an Intel Core i9-13900K.

## Using the crate

```rust
use shimeji::{Lsh256_256, Lsh512_512};

// A whole message in one call.
let digest: [u8; 32] = Lsh256_256::digest(b"abc");

// A message that arrives in parts.
let mut hasher = Lsh512_512::new();
hasher.update(b"a");
hasher.update(b"bc");
let digest: [u8; 64] = hasher.finalize();
```

The digest length in bytes is a type parameter. `Lsh256<28>` and
`Lsh256<32>` are LSH-256-224 and LSH-256-256, and `Lsh512<28>`, `<32>`,
`<48>` and `<64>` are LSH-512-224, -256, -384 and -512. The aliases
`Lsh256_224` to `Lsh512_512` name the six. The crate is `no_std` and has no
dependencies.

## Testing

- **Known-answer tests.** `tests/kat.rs` checks the 735 test vectors that
  Crypto++ publishes for LSH-256-224, LSH-256-256, LSH-512-256, LSH-512-384
  and LSH-512-512. Each message is hashed in one piece and again in chunks of
  1, 3, 127, 128, 129, 255, 256 and 257 bytes, which cross the 128-byte
  LSH-256 and 256-byte LSH-512 block boundaries.
- **Differential test.** `harness/` compiles KISA's reference C code once per
  variant and links the variants into one binary: the portable code on every
  machine, NEON on aarch64, and SSE2, SSSE3 and AVX2 on x86-64.
  `harness/tests/differential.rs` hashes every length from 0 to 3000 bytes,
  plus 65,535, 65,536, 65,537 bytes and 1 MiB, at varying buffer offsets,
  with each variant the CPU supports and all six digests, and requires the
  digest shimeji computes. LSH-512-224 has no published test vector, so this
  comparison is its check.

## Building

`nix develop` opens a shell with the Rust toolchain, clang, and typst with the
TeX Gyre fonts the paper is set in. Without nix, the code needs a recent stable
Rust toolchain and clang for the harness.

```sh
# known-answer tests, documentation examples and the differential test
cargo test --workspace --release

# the paper
typst compile --root paper paper/main.typ PAPER.pdf

# download the reference code and test vectors again, then check them
# against refs/SHA256SUMS
refs/fetch.sh
```

## Repository layout

```text
src/        the crate: the LSH construction and the parameters of LSH-256 and LSH-512
tests/      known-answer tests
harness/    KISA's C code, built side by side, and the differential test
refs/       KISA's LSH code and the Crypto++ test vectors, pinned, with checksums
paper/      typst sources and bibliography of the paper
PAPER.pdf   the compiled paper
flake.nix   the nix development shell
```

## License

shimeji is distributed under the MIT license, in `LICENSE`. The files in
`refs/` keep their own terms. KISA's code is copyright 2016 NSR under the MIT
license, and Crypto++ places its test vectors in the public domain.
`refs/readme.md` gives the sources.
