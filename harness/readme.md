# harness

checks that shimeji's digests equal those of other LSH implementations, and
measures how fast each one hashes. run everything from `nix develop`.

```sh
cargo test -p harness --release                    # differential test
cargo run -p harness --release -- bench            # writes results/<host>-<time>.tsv
cargo run -p harness --release -- summary results/<file>.tsv
```

## implementations

`src/lib.rs` holds one registry, `IMPLEMENTATIONS`: each entry has a name, a
family (LSH-256 or LSH-512), the digest lengths it computes, a function that
hashes a message into an output slice whose length selects the digest, and a
check that the running CPU can execute it. the test and the benchmark iterate
the registry and name no implementation, so a new backend is one more entry.

it holds shimeji's code and KISA's C code from `refs/kisa`: `ref` everywhere,
`neon` on aarch64, `sse2`, `ssse3` and `avx2` on x86_64. `shimeji` is what
`digest` runs, the fastest code the CPU supports; `shimeji-portable` is the
portable code alone. on x86_64, `shimeji-ssse3` and `shimeji-avx2` call each
one-message SIMD kernel directly, whatever `digest` would pick.
KISA's `xop` variant needs AMD XOP hardware and is not built.

every KISA variant defines the same four functions. `build.rs` compiles each
variant from a generated file that renames them with `#define` to
`kisa_<variant>_lsh256_digest` and so on, then includes the vendored
`lsh-impl.c` unchanged, and adds `kisa_<variant>_lsh256_repeat` (and the
LSH-512 one), which calls the digest function a given number of times. the
benchmark times that loop (`repeat`, below): calling KISA's code once per
64-byte message from Rust, through a buffer aligned for its SIMD variants,
adds 1-6% to its time on an Apple M6 and an Intel i9-13900K. clang rejects KISA's LSH-512 NEON code, which passes a
shift count to an immediate-operand intrinsic through an inline function;
`c/neon_imm.h` turns those intrinsics into switches that fold to the same
`shl` and `sri` instructions GCC emits.

the C compiler and its flags are set in one place, `VARIANTS` and `CFLAGS` in
`build.rs`: clang with `-O3 -fPIC`, plus `-msse2`, `-mssse3` or `-mavx2`.
`build.rs` writes the full command lines, with the rustc version, `RUSTFLAGS`
and the release profile, to the header of every result file.

on x86_64 it also holds the SSE2, SSSE3 and AVX2 code of Kim, Jung, Ju and
Song, "Fast Implementation of LSH With SIMD" (IEEE Access, 2019), from
`refs/lsh-simd`: `lsh-simd-sse2`, `lsh-simd-ssse3` and `lsh-simd-avx2`, each
with all six digests. its variants also share function names, so `build.rs`
compiles them the same way as KISA's, renamed to `lsh_simd_<variant>_*`,
with the same clang and flags, and with a C loop around `Hash256` or
`Hash512` that the benchmark times: called once per 64-byte message from
Rust, the code takes 1-1.5% longer on an i9-13900K. for every digest but
LSH-256-256 and LSH-512-512, this code derives the IV for each message by
running the step functions without a message, where KISA's code stores it.

`pbkdf2-lsh-sse2`, on x86_64, is the SSE2 LSH-256 that Choi and Seo wrote
for their PBKDF2-HMAC-LSH256 (https://github.com/ondoli0312/PBKDF2-HMAC-LSH256,
`SIMD_LSH_256.c`); the copies of KISA's code in that repository are not
registered. it computes LSH-256-256 only. the repository has no license, so
its code is not in this one: `build.rs` downloads the three files it needs
at a pinned commit with `curl` into `refs/pbkdf2-lsh/`, which git ignores,
checks each against a pinned SHA-256, and stops with a message naming the
files if it cannot. it is compiled with KISA's flags plus `-msse2` and
`-fms-extensions`, which the code's `__declspec(align)` and `__int64` need,
from a generated unit that works around a prototype in `LSH256.h` that
disagrees with the definition, and the benchmark times a C loop around it,
as for `lsh-simd`: called once per 64-byte message from Rust it takes 1.7%
longer on an i9-13900K.

Crypto++ (https://github.com/weidai11/cryptopp) is built from the commit
`flake.nix` pins, the one `refs/fetch.sh` takes the test vectors from, with
its own GNUmakefile, the same clang as KISA's code and `-O3 -fPIC -DNDEBUG`;
the GNUmakefile adds `-mssse3` and `-mavx2` to its SSSE3 and AVX2 LSH files.
`c/cryptopp.cpp` exports its LSH classes to C, one object per digest and
thread, as `CalculateDigest` leaves it ready for the next message. Crypto++
picks portable C++, SSSE3 or AVX2 code at run time, and each entry is
available when Crypto++'s `AlgorithmProvider` names its path:
`cryptopp-ref`, `cryptopp-ssse3` or `cryptopp-avx2`. it has no LSH-512-224
class; the shim derives one from `LSH512_Base` with KISA's algorithm type
for it. `build.rs` records the source, the make command and the compile
lines of the LSH files in the result header.

digestpp (https://github.com/kerukuro/digestpp), a header-only C++ hash
library, is pinned by `flake.nix` and compiled into `c/digestpp.cpp` with
the Crypto++ shim's flags. its LSH is portable C++ with all six digests,
registered as `digestpp` on every machine. the shim hashes each message with
a new hasher, `digestpp::lsh256(256).absorb(msg, len).digest(out, 32)`,
which first derives the IV with one compression function call on a zero
block. digestpp's `rotate_left` computes `x >> (32 - n)`, undefined for the
rotation by 0 in LSH's gamma step, and clang at `-O1` or above then returns
wrong digests, also for digestpp's own test vectors; `flake.nix` applies
`c/digestpp-rotate.patch`, which masks the shift count. a C++ loop instead
of one call per 64-byte message changes its time by less than 0.5% on an
i9-13900K, so it has no `repeat`.

`lsh256-rs` is the `lsh256` crate
(https://github.com/corp0ratestarts-cmd/LSH-256-Korea-KS-X-3262-RUST),
portable Rust without `unsafe` and with all six digests, a git dependency
in `Cargo.toml` pinned by commit and built with the workspace's release
profile. its one-shot functions each allocate the digest as a `Vec`.

go-krypto (https://github.com/RyuaNerin/go-krypto, Go with assembly) is
pinned to v1.3.0 by `go/go.mod` and `go/go.sum`; the first build downloads
it through the Go module proxy. it is built with Go 1.26, because its amd64
assembly loads its IVs with aligned SSE instructions from Go slices that Go
1.27 places at 8-byte alignment, which faults. `go/gokrypto.go` exports
its `Sum224`, `Sum256`, `Sum384` and `Sum512` to C, and `build.rs` builds it
with `go build -buildmode=c-archive` and links the archive. go-krypto picks
its assembly from the CPU when the Go runtime starts, and each entry is named
after the path it takes: `go-krypto-neon` on aarch64; on x86_64,
`go-krypto-ssse3` (or `-sse2`) for LSH-256, whose AVX2 code go-krypto leaves
disabled as slower, and `go-krypto-avx2` (or `-ssse3`, `-sse2`) for LSH-512.
a call into Go through cgo costs tens of nanoseconds, so these entries hash in
a loop inside Go when benchmarked (`repeat`, below). `build.rs` records the Go
version, the go-krypto version and the build command in the result header.

## differential test

`tests/differential.rs` hashes every message length from 0 to 3000 bytes,
plus lengths around 64 KiB and 1 MiB, with every registry entry and all six
digests, and requires each digest to equal `shimeji-portable`'s. messages
come from a fixed seed and start at varying offsets. LSH-512-224 has no
Crypto++ test vector, so this test is its only check.

## benchmark

`bench` takes `--impl`, `--digest` and `--len` as comma-separated lists
(default: every available implementation, all six digests, and 16, 64, 256,
1024, 4096, 16384 and 1048576 bytes), `--reps` (20), `--min-time-ms` (20),
`--cpu` and `--out`.

- the run warms up for 500 ms. then, for each digest and length, each
  implementation is calibrated so one timed loop of hashes lasts at least the
  minimum time, far above the timer's resolution.
- repetitions alternate the order of implementations (A B, B A, ...), so a
  slow drift in clock speed falls on all of them alike.
- the message, the output and the function pointer pass through
  `black_box`, so every hash is an opaque call whose result is used.
- an entry may also have `repeat`, a loop in its own language that hashes
  the message a given number of times; the benchmark then times that loop
  instead of calling `hash` once per message. it is for code behind a
  foreign call that costs as much as hashing a short message, such as Go
  called through cgo. the differential test always uses `hash`.
- on macOS the thread runs at user-interactive QoS and the core is recorded
  after each repetition; macOS cannot pin threads. on Linux, `--cpu N` pins
  the thread to CPU N and the core is recorded the same way.

each repetition is one row: implementation, digest, length, repetition,
ns per byte, ns per message, cycles per byte, retired instructions per byte,
CPU, host. comment lines above the rows record host, OS, CPU model, git
commit, command line, method, how cycles and instructions were counted, and
build flags.

cycles and instructions are read from the hardware counters of the
benchmark's thread just before and after each timed loop:

- on Linux, with `perf_event_open`: the generic cycle and instruction events
  as one group, user mode only. a repetition whose counters did not run for
  the whole loop (multiplexed, or on a core whose PMU lacks them) gets empty
  columns.
- on macOS, with `thread_selfcounts`, which returns the core's fixed cycle
  and instruction counters as accumulated for the calling thread. it needs
  no privileges and counts user and kernel mode.

where neither works, the columns are empty and the header says why.

`summary` prints, per digest, length and implementation, the median with a
95% bootstrap interval, the median cycles and instructions per byte, and the
speedup over `--baseline` (default `kisa-ref`).
`--cpus 6,7` keeps only repetitions that ended on those cores, to drop runs
that migrated between core clusters.
