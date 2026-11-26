#import "/lib.typ": *

= Method <method>

#let hm = n.machines.m6.header
#let hi = n.machines.i9.header
#let go-clock(lo, hi) = lengths.filter(l => l >= lo and l <= hi).map(l => n.machines.i9.go_clock_ghz.at(str(l)))

We measured on two machines (@tab:machines). The Apple M6 has three kinds of core, and macOS cannot pin a thread to one. The harness therefore runs at the user-interactive quality of service, which macOS schedules on its fastest cores. It records the core of every repetition and keeps the repetitions that ran on the two Super cores, #num(n.machines.m6.repetitions) of #num(n.machines.m6.repetitions_all). The Intel Core i9-13900K ran in a bench mode: Turbo Boost off, every core at its 3.0 GHz base clock under the performance governor, and interrupts moved off CPUs 8 to 15. The kernel also keeps CPUs 8 to 15 free of other tasks, timer ticks and RCU callbacks (`isolcpus`, `nohz_full`, `rcu_nocbs`). The harness ran pinned to CPU 8, one thread of a P-core whose other thread, CPU 9, stayed idle. Cycles divided by time give a median clock of #r2(n.machines.i9.clock_ghz) GHz for every implementation except go-krypto.

#figure(
  table(
    columns: (auto, 1fr, 1fr),
    align: (left, left, left),
    stroke: none,
    table.hline(),
    table.header([], [Apple M6], [Intel Core i9-13900K]),
    table.hline(stroke: 0.5pt),
    [Cores], [2 Super, 4 P, 6 E], [8 P (2 threads each), 16 E],
    [Clock], [not fixed; #r2(n.machines.m6.clock_ghz) GHz median], [3.0 GHz base, fixed],
    [System], [macOS 27.2], [NixOS 26.11, Linux 7.3.4],
    [Runs on], [Super cores, by QoS], [CPU 8, isolated, pinned],
    [Counters], [`thread_selfcounts`, user and kernel], [`perf_event_open`, user],
    [Rust], table.cell(colspan: 2)[rustc #hm.rustc.split(" ").at(1) (2026-11-11), opt-level 3, 1 codegen unit, fat LTO],
    [C, C++], table.cell(colspan: 2)[clang #hm.cc.split(" ").at(2), `-O3 -fPIC`],
    [Go], table.cell(colspan: 2)[#hm.go.split(" ").at(2).slice(2)],
    table.hline(),
  ),
  caption: [The two machines and how the benchmark ran on each. P and E are performance and efficiency cores.],
) <tab:machines>

Mytkowicz et al. showed that the link order and the size of the environment alone can change a measured time by several percent @mytkowicz2009wrong. Every implementation runs from one binary in one process, so all of them see the same environment and the same initial stack alignment. Every comparison in @results also rests first on retired instructions, which code layout does not change.

== Implementations

@tab:impls lists the implementations, the version each was built from and the code each runs. All C and C++ code is built by the same clang with `-O3 -fPIC`, plus `-msse2`, `-mssse3` or `-mavx2` for the SIMD variants. All Rust code is built by the same rustc with shimeji's release profile.

#figure(
  table(
    columns: (auto, auto, auto, auto, 1fr),
    align: (left, left, left, left, left),
    stroke: none,
    table.hline(),
    table.header([], [Lang.], [Version], [M6], [i9]),
    table.hline(stroke: 0.5pt),
    [shimeji], [Rust], [#hm.commit.slice(0, 7)], [hybrid, portable], [AVX2, SSSE3, portable],
    [KISA @kisa2026lsh], [C], [200cd30], [C, NEON], [C, SSE2, SSSE3, AVX2],
    [Kim et al. @kim2019simd], [C], [7bf958f], [–], [SSE2, SSSE3, AVX2],
    [Choi–Seo @choi2020code], [C], [be7026c], [–], [SSE2],
    [Crypto++ @dai2026cryptopp], [C++], [5873a73], [portable], [AVX2],
    [digestpp @kerukuro2026digestpp], [C++], [4beae75], [portable], [portable],
    [lsh256 crate @lsh256rs2026], [Rust], [4dea1cc], [portable], [portable],
    [allcrypt @allcrypt2026], [Rust], [0.8.1], [portable], [portable],
    [go-krypto @ryuanerin2026gokrypto], [Go], [#hm.at("go-krypto").split(" ").at(1)], [NEON], [SSSE3, AVX2],
    table.hline(),
  ),
  caption: [The implementations, the version or commit each was built from, and the code paths measured on each machine. KISA's commit is that of the SUPERCOP mirror. A dash marks code that does not run there.],
) <tab:impls>

We use KISA's code as packaged in SUPERCOP, which has portable C, SSE2, SSSE3, AVX2, XOP and NEON variants. XOP needs AMD hardware that neither machine has, so neither KISA's nor Kim et al.’s XOP variant ran. clang rejects KISA's LSH-512 NEON code, which passes a shift count through an inline function to an intrinsic that requires an immediate. A header of ours turns those intrinsics into switches that compile to the same `shl` and `sri` instructions that GCC emits. For every digest but LSH-256-256 and LSH-512-512, Kim et al.’s code derives the IV of each message by running the steps on an empty state, while the other implementations store the IV. We report those two digests. Choi and Seo wrote their SSE2 LSH-256 for PBKDF2-HMAC-LSH256 @choi2021pbkdf2, and it computes LSH-256-256 only. Its repository has no license, so its code is not in ours. The build downloads it at a pinned commit and checks it against pinned SHA-256 digests. Crypto++ picks portable, SSSE3 or AVX2 code at run time. It has no class for LSH-512-224, so our shim derives one from its LSH-512 base class with KISA's algorithm type. digestpp rotates by computing `x >> (32 - n)`, which C++ leaves undefined for the rotation by zero in $gamma$. clang 21 then returns wrong digests at `-O1` and above, also for digestpp's own test vectors. We patch its rotation to mask the shift count. The lsh256 crate and allcrypt are portable Rust without `unsafe`, and both return each digest in a newly allocated `Vec`. go-krypto needs Go 1.26. Its amd64 assembly loads the IVs with aligned SSE instructions from Go slices that Go 1.27 aligns to eight bytes only, and the loads fault. It picks its assembly when the Go runtime starts. On the i9 it takes AVX2 code for LSH-512 and SSSE3 code for LSH-256, whose AVX2 code it leaves disabled.

== Harness and statistics

The harness holds every implementation in one registry, which the differential test and the benchmark both iterate. An entry hashes messages through a function pointer. The message, the output and the pointer pass through `black_box`, so every hash is an opaque call whose result is used. The benchmark measures each digest at message lengths of 16, 64, 256, 1024, 4096 and 16384 bytes and 1 MiB. For each digest and length, it calibrates every implementation so that one timed loop of hashes lasts at least 20 ms. After 500 ms of warm-up it runs #n.machines.m6.reps repetitions of each implementation, alternating their order (A B, B A, ...) so that a slow drift of clock or temperature affects all of them alike. A call into C through Rust's foreign function interface, or into Go through cgo, takes time comparable to hashing a short message. The entries of KISA, Kim et al., Choi and Seo, and go-krypto therefore time a loop in their own language.

go-krypto's loop runs inside the Go runtime, which takes the timed thread off the CPU for part of the loop. Its times are wall time, like those of every other implementation. On the i9, however, its cycle counter counts only the time the thread spends on the CPU. Its counted cycles over wall time give a median clock of #r1(calc.min(..go-clock(16, 4096))) to #r1(calc.max(..go-clock(16, 4096))) GHz on messages up to 4 KiB, so its counted cycles understate its cost there. We therefore give cycles on the i9 as time at the fixed clock, for every implementation. Instructions per byte and per cycle come straight from the counters. @results quotes them for go-krypto only at 1 MiB, where its clock is #r2(n.machines.i9.go_clock_ghz.at("1048576")) GHz.

The harness reads the thread's cycle and instruction counters just before and after each timed loop. On Linux it uses `perf_event_open` with the generic cycle and instruction events as one group, counting user mode. On macOS it uses `thread_selfcounts`, which returns the core's fixed counters as accumulated for the calling thread and needs no privileges. It counts user and kernel mode. We report medians with 95% percentile-bootstrap intervals from 10,000 resamples. For a ratio of two medians, the two series are resampled independently.
