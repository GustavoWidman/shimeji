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

it holds shimeji's portable code and KISA's C code from `refs/kisa`: `ref`
everywhere, `neon` on aarch64, `sse2`, `ssse3` and `avx2` on x86_64. KISA's
`xop` variant needs AMD XOP hardware and is not built.

every KISA variant defines the same four functions. `build.rs` compiles each
variant from a generated file that renames them with `#define` to
`kisa_<variant>_lsh256_digest` and so on, then includes the vendored
`lsh-impl.c` unchanged. clang rejects KISA's LSH-512 NEON code, which passes a
shift count to an immediate-operand intrinsic through an inline function;
`c/neon_imm.h` turns those intrinsics into switches that fold to the same
`shl` and `sri` instructions GCC emits.

the C compiler and its flags are set in one place, `VARIANTS` and `CFLAGS` in
`build.rs`: clang with `-O3 -fPIC`, plus `-msse2`, `-mssse3` or `-mavx2`.
`build.rs` writes the full command lines, with the rustc version, `RUSTFLAGS`
and the release profile, to the header of every result file.

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
digests, and requires each digest to equal shimeji's portable code. messages
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
ns per byte, ns per message, CPU, host. comment lines above the rows record
host, OS, CPU model, git commit, command line, method and build flags.

`summary` prints, per digest, length and implementation, the median with a
95% bootstrap interval and the speedup over `--baseline` (default `kisa-ref`).
`--cpus 6,7` keeps only repetitions that ended on those cores, to drop runs
that migrated between core clusters.
