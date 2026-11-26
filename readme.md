# shimeji

An implementation of LSH-256 and LSH-512, the hash functions of the Korean
standard KS X 3262, in Rust.

Gustavo Wagon Widman and João Victor Wandermurem, Computer Engineering,
Instituto de Tecnologia e Liderança (Inteli), São Paulo, Brazil. Advised by
Bryan Kano Ferreira.

The paper is [PAPER.pdf](PAPER.pdf). Its typst sources are in `paper/`.

## Overview

LSH is a family of wide-pipe Merkle–Damgård hash functions. Korean
cryptography uses it where other standards use SHA-2, in PBKDF2 and in the
hash-based signature schemes K-XMSS, K-SPHINCS+ and K-LMS. Its optimized
implementations are written in C (KISA's reference code), C++ (Crypto++) and
Go (go-krypto). The Rust implementations, among them the allcrypt and lsh256
crates, are portable code without SIMD.

shimeji implements both functions and all six digest lengths in Rust, with a
compression function for each kind of CPU:

- On little-endian aarch64, the steps run on the integer units while NEON
  registers hold and expand the message schedule.
- On x86-64, the state lives in SSSE3 or AVX2 registers, in lane orders that
  keep word permutations off the critical path of each step. The crate reads
  the CPU's features with `cpuid` on first use and picks AVX2, then SSSE3,
  then the portable code.
- Elsewhere, the portable code runs.

The paper describes the implementation and measures it against eight other
implementations in C, C++, Go and Rust on an Apple M6 and an Intel Core
i9-13900K.

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
`Lsh256_224` to `Lsh512_512` name the six, and any other length fails to
compile. The crate is `no_std` and has no dependencies.

## Testing

- **Known-answer tests.** `tests/kat.rs` checks the 735 test vectors that
  Crypto++ publishes for LSH-256-224, LSH-256-256, LSH-512-256, LSH-512-384
  and LSH-512-512. Each message is hashed in one call, in one `update`, and
  in eight sequences of `update` calls cut at seeded random offsets, about
  half of them within two bytes of a block boundary. `KAT_SEED` sets the
  seed.
- **Differential test.** The `harness` crate links the other
  implementations into one binary: KISA's C code in each of its variants,
  Kim et al.'s and Choi and Seo's SIMD C code, Crypto++, digestpp,
  go-krypto, the lsh256 crate and allcrypt. `harness/tests/differential.rs`
  hashes every length from 0 to 3000 bytes, plus lengths around 64 KiB and
  1 MiB, at varying buffer offsets, with every implementation and code path
  the CPU runs and all six digests, and requires the digest of shimeji's
  portable code. LSH-512-224 has no published test vector, so this
  comparison is its check.

## Benchmarks

`harness bench` times every implementation on every digest at message
lengths from 16 bytes to 1 MiB. Each repetition is one loop of at least
20 ms; the implementations alternate in ABBA order, and the thread's cycle
and instruction counters are read around each loop. `harness summary`
reduces a results file to medians with 95% bootstrap intervals, and
`harness footprint` prints the state and stack each implementation uses.
`harness/readme.md` describes each implementation's build and the method in
detail.

`results/` holds the runs the paper reports, each with a readme that gives
the machine, its settings and the commands. `paper/analysis/analyze.py`
turns them into the numbers and the figure the paper prints.

## Building

`nix develop` opens a shell with the pinned Rust nightly toolchain, clang,
Go 1.26, the pinned Crypto++ and digestpp sources, Python with matplotlib,
and typst with the TeX Gyre fonts the paper is set in. The crate uses no
nightly features, and `nix develop .#stable` opens the same shell with
stable Rust.

```sh
# known-answer tests, documentation examples and the differential test
cargo test --workspace --release

# a benchmark run, and its summary
cargo run -p harness --release -- bench --out results/run.tsv
cargo run -p harness --release -- summary results/run.tsv

# the numbers, the figure and the paper
python3 paper/analysis/analyze.py
typst compile --root paper paper/main.typ PAPER.pdf

# download the reference code and test vectors again, then check them
# against refs/SHA256SUMS
refs/fetch.sh
```

## Repository layout

```text
src/        the crate: the LSH construction, the parameters of LSH-256 and
            LSH-512, and the NEON and x86-64 code
tests/      known-answer tests
harness/    the other implementations, built side by side, the differential
            test and the benchmark
refs/       KISA's LSH code, Kim et al.'s SIMD code and the Crypto++ test
            vectors, pinned, with checksums
results/    the benchmark runs the paper reports
paper/      typst sources, bibliography, analysis scripts and figures
PAPER.pdf   the compiled paper
flake.nix   the nix development shell
```

## License

shimeji is distributed under the MIT license, in `LICENSE`. The files in
`refs/` keep their own terms. KISA's code is copyright 2016 NSR under the MIT
license, Kim et al.'s code is copyright 2019 JuYoungjin under the MIT
license, and Crypto++ places its test vectors in the public domain. Choi and
Seo's code has no license, so the harness downloads it at a pinned commit
for each build instead of keeping it here. `refs/readme.md` gives the
sources.
