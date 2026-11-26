#import "/lib.typ": *

= Results for one message <results>

#let fams = ("lsh256", "lsh512")
#let big = 1048576
#let fewer(m, fam, imp, other) = 1 - ipb(m, fam, big, imp) / ipb(m, fam, big, other)
#let spread(m) = n.spread.at(m).values().map(f => f.values()).flatten()
#let worst(m, fam) = calc.min(..lengths.map(l => best(m, fam, l).ratio))
#let slower(m, fam) = lengths.filter(l => best(m, fam, l).ratio < 1)
#let widest = lengths.sorted(key: l => best("i9", "lsh256", l).ratio).last()

We report LSH-256-256 and LSH-512-512. The other four digests differ from them only in IV and output length. Over all six digests, shimeji's geometric-mean speedup over the fastest other implementation lies between #r2(calc.min(..spread("m6"))) and #r2(calc.max(..spread("m6"))) on the M6 and between #r2(calc.min(..spread("i9"))) and #r2(calc.max(..spread("i9"))) on the i9. This section shows that:

- on the M6, shimeji retires #pct(fewer("m6", "lsh256", "shimeji", "kisa-ref")) and #pct(fewer("m6", "lsh512", "shimeji", "kisa-ref")) fewer instructions per byte than KISA's C code, at a lower rate, and the NEON-only implementations retire fewer instructions than KISA's C but at under half its rate (@ins);#for fam in fams { for i in ("kisa-neon", "go-krypto-neon") { assert(ipc("m6", fam, big, i) < ipc("m6", fam, big, "kisa-ref") / 2) } }
- on the i9, shimeji's AVX2 build of LSH-256 retires fewer instructions than any other code that keeps the state in 128-bit registers, and the 256-bit code of KISA and Kim et al. retires fewer still but at about half the rate (@ins);#for i in ("kisa-avx2", "lsh-simd-avx2") { for j in ("shimeji-ssse3", "kisa-ssse3", "lsh-simd-ssse3", "go-krypto-ssse3") { let r = ipc("i9", "lsh256", big, i) / ipc("i9", "lsh256", big, j); assert(r > 0.4 and r < 0.6) } }
- on the M6 this makes shimeji #r2(geo("m6", "lsh256")) and #r2(geo("m6", "lsh512")) times as fast as KISA's C, the fastest other implementation at every length, as geometric means over the seven lengths (@cycles);
- on the i9 shimeji is #r2(geo("i9", "lsh256")) and #r2(geo("i9", "lsh512")) times as fast as the fastest other implementation at each length, again as geometric means (@cycles);
- shimeji's state takes as many bytes as KISA's context, its peak stack is smaller, and its unrolled code is larger (@footprint).

== Instructions per byte <ins>

#let rows = (
  m6: ("shimeji", "shimeji-portable", "kisa-ref", "kisa-neon", "go-krypto-neon", "cryptopp-ref", "digestpp", "lsh256-rs", "allcrypt"),
  i9: ("shimeji", "shimeji-ssse3", "shimeji-portable", "kisa-ref", "kisa-sse2", "kisa-ssse3", "kisa-avx2",
    "lsh-simd-sse2", "lsh-simd-ssse3", "lsh-simd-avx2", "pbkdf2-lsh-sse2", "cryptopp-avx2", "digestpp", "lsh256-rs", "allcrypt", "go-krypto"),
)
// go-krypto takes a different code path per family on the i9.
#let entry(m, fam, imp) = if imp == "go-krypto" { if fam == "lsh256" { "go-krypto-ssse3" } else { "go-krypto-avx2" } } else { imp }
#let has(m, fam, imp) = entry(m, fam, imp) in n.cells.at(m).at(fam).at(str(big))
#let min-reps = calc.min(..fams.map(fam => n.cells.m6.at(fam).values().map(c => c.values().map(x => x.n)).flatten()).flatten())
#let name(m, imp) = if imp == "go-krypto" { "go-krypto" } else if m == "i9" and imp == "shimeji" { "shimeji, AVX2" } else if imp == "shimeji" { "shimeji, hybrid" } else { label.at(imp) }

#figure(
  table(
    columns: (auto, auto, auto, auto, auto, auto, auto),
    align: (left, right, right, right, right, right, right),
    stroke: none,
    table.hline(),
    table.header(
      [], table.cell(colspan: 3, align: center)[LSH-256-256], table.cell(colspan: 3, align: center)[LSH-512-512],
    ),
    table.hline(start: 1, end: 4, stroke: 0.5pt), table.hline(start: 4, end: 7, stroke: 0.5pt),
    [], [ins./B], [IPC], [cyc./B], [ins./B], [IPC], [cyc./B],
    ..("m6", "i9").map(m => (
      table.hline(stroke: 0.5pt),
      table.cell(colspan: 7)[_#if m == "m6" [Apple M6] else [Intel Core i9-13900K]_],
      ..rows.at(m).map(imp => (
        [#name(m, imp)],
        ..fams.map(fam => if has(m, fam, imp) {
          let e = entry(m, fam, imp)
          ([#r2(ipb(m, fam, big, e))], [#r2(ipc(m, fam, big, e))], [#r2(cpb(m, fam, big, e))])
        } else { ([–], [–], [–]) }).flatten(),
      )).flatten(),
    )).flatten(),
    table.hline(),
  ),
  caption: [Retired instructions per byte, instructions per cycle (IPC) and cycles per byte of every implementation on one message of 1 MiB. Values are medians of #n.machines.m6.reps repetitions on the i9 and of the #min-reps to #n.machines.m6.reps repetitions kept on the M6. go-krypto runs SSSE3 code for LSH-256 and AVX2 code for LSH-512 on the i9.],
) <tab:ins>

#let lo(m, fam, imps, f) = calc.min(..imps.map(i => f(m, fam, big, i)))
#let hi(m, fam, imps, f) = calc.max(..imps.map(i => f(m, fam, big, i)))
#let integer = ("kisa-ref", "shimeji-portable", "shimeji")
#let portable = ("cryptopp-ref", "digestpp", "lsh256-rs", "allcrypt")
#let nearest = portable.sorted(key: i => ns("m6", "lsh256", big, i)).first()
#let ssse3 = ("shimeji-ssse3", "kisa-ssse3", "lsh-simd-ssse3", "go-krypto-ssse3")
#let avx2-512 = ("shimeji", "kisa-avx2", "lsh-simd-avx2", "go-krypto-avx2")
#let rate(m, imps) = (
  calc.min(..fams.map(f => lo(m, f, imps, ipc))),
  calc.max(..fams.map(f => hi(m, f, imps, ipc))),
)
#let less(fam) = 1 - cpb("m6", fam, big, "shimeji") / cpb("m6", fam, big, "kisa-ref")
#assert(ipb("i9", "lsh256", big, "shimeji") < lo("i9", "lsh256", ssse3, ipb))

@tab:ins gives the instructions every implementation retires per byte of a long message, and the rate at which it retires them. On the M6, KISA's C and shimeji's portable and hybrid paths run the steps on the integer units. They retire #r1(rate("m6", integer).at(0)) to #r1(rate("m6", integer).at(1)) instructions per cycle, and their speed follows their instruction count. shimeji's hybrid path retires #pct(fewer("m6", "lsh256", "shimeji", "kisa-ref")) fewer instructions per byte of LSH-256 than KISA's C and #pct(fewer("m6", "lsh512", "shimeji", "kisa-ref")) fewer of LSH-512, and takes #if pct(less("lsh256")) == pct(less("lsh512")) [#pct(less("lsh256")) fewer cycles on both] else [#pct(less("lsh256")) and #pct(less("lsh512")) fewer cycles]. The NEON code of KISA and go-krypto retires fewer instructions than KISA's C. KISA's NEON code retires #r1(ipb("m6", "lsh256", big, "kisa-neon")) and #r1(ipb("m6", "lsh512", big, "kisa-neon")) per byte, but at #r1(ipc("m6", "lsh256", big, "kisa-neon")) and #r1(ipc("m6", "lsh512", big, "kisa-neon")) per cycle, the latency-bound rate that @hybrid explains. It needs #r2(cpb("m6", "lsh256", big, "kisa-neon")) and #r2(cpb("m6", "lsh512", big, "kisa-neon")) cycles per byte, against #r2(cpb("m6", "lsh256", big, "kisa-ref")) and #r2(cpb("m6", "lsh512", big, "kisa-ref")) for KISA's C. The portable libraries retire their instructions at #r1(rate("m6", portable).at(0)) to #r1(rate("m6", portable).at(1)) per cycle. The fastest of them on LSH-256, #label.at(nearest), takes #r2(ns("m6", "lsh256", big, nearest) / ns("m6", "lsh256", big, "kisa-ref")) times as long as KISA's C. Of the two Rust crates, allcrypt is the faster, and shimeji's portable code alone is #r2(speedup("m6", "lsh256", big, "shimeji-portable", "allcrypt")) and #r2(speedup("m6", "lsh512", big, "shimeji-portable", "allcrypt")) times as fast as allcrypt on long messages of LSH-256 and LSH-512.#for fam in fams { assert(ns("m6", fam, big, "allcrypt") < ns("m6", fam, big, "lsh256-rs")) }

On the i9, the SSSE3 code of shimeji, KISA, Kim et al. and go-krypto retires #r1(lo("i9", "lsh256", ssse3, ipb)) to #r1(hi("i9", "lsh256", ssse3, ipb)) instructions per byte of LSH-256 at #r1(lo("i9", "lsh256", ssse3, ipc)) to #r1(hi("i9", "lsh256", ssse3, ipc)) per cycle. shimeji's SSSE3 build has the highest rate of the four, since no permute sits on its critical path.#assert(ssse3.slice(1).all(i => ipc("i9", "lsh256", big, i) < ipc("i9", "lsh256", big, "shimeji-ssse3"))) Its AVX2 build retires #r1(ipb("i9", "lsh256", big, "shimeji")) instructions per byte, fewer than any of them, because VEX encoding removes its register copies. KISA's and Kim et al.’s AVX2 code retire only #r1(ipb("i9", "lsh256", big, "kisa-avx2")) and #r1(ipb("i9", "lsh256", big, "lsh-simd-avx2")), but at #r1(ipc("i9", "lsh256", big, "kisa-avx2")) and #r1(ipc("i9", "lsh256", big, "lsh-simd-avx2")) per cycle, since their lane-crossing permutes sit on the critical path of every step. Both run slower than their own SSSE3 code.#for v in ("kisa", "lsh-simd") { assert(cpb("i9", "lsh256", big, v + "-avx2") > cpb("i9", "lsh256", big, v + "-ssse3")) } For LSH-512 the AVX2 kernels of shimeji, KISA, Kim et al. and go-krypto retire #r1(lo("i9", "lsh512", avx2-512, ipb)) to #r1(hi("i9", "lsh512", avx2-512, ipb)) instructions per byte and take #r2(lo("i9", "lsh512", avx2-512, cpb)) to #r2(hi("i9", "lsh512", avx2-512, cpb)) cycles per byte. All four spend #r1(lo("i9", "lsh512", avx2-512, cpb) * block.lsh512 / steps.lsh512) to #r1(hi("i9", "lsh512", avx2-512, cpb) * block.lsh512 / steps.lsh512) cycles per step on the chain of dependent instructions of the mix function and the permutes between steps. shimeji's SSSE3 kernel for LSH-512, which runs only on CPUs without AVX2, is slower than KISA's and Kim et al.’s SSSE3 code.#assert(cpb("i9", "lsh512", big, "shimeji-ssse3") > calc.max(cpb("i9", "lsh512", big, "kisa-ssse3"), cpb("i9", "lsh512", big, "lsh-simd-ssse3")))

== Cycles per byte <cycles>

@fig:cpb shows cycles per byte against message length. @tab:ratio gives shimeji's speedup over the fastest other implementation at each length.

#figure(
  placement: auto,
  image("/figures/cycles-per-byte.svg", width: 100%),
  caption: [Cycles per byte of one message against its length for LSH-256-256 (a, c) and LSH-512-512 (b, d), on the M6 (a, b) and the i9 (c, d); lower is better. Each panel shows shimeji, its portable code and the three fastest other implementations. Cycles on the i9 are time at its fixed 3.0 GHz clock.],
) <fig:cpb>

#let letters = (:)
#let marks = ("a", "b", "c", "d", "e", "f", "g")
#let names = ()
#for m in ("m6", "i9") { for fam in fams { for l in lengths { names.push(best(m, fam, l).name) } } }
#let names = names.dedup()
#let mark(name) = marks.at(names.position(x => x == name))
#let ratio-cell(m, fam, l) = {
  let b = best(m, fam, l)
  if b.close {
    stack(spacing: 1.5pt, [#r2(b.ratio)#super[#mark(b.name)]], text(size: 6pt)[\[#fix(b.lo, 3), #fix(b.hi, 3)\]])
  } else [#r2(b.ratio)#super[#mark(b.name)]]
}

#figure(
  table(
    columns: (auto, 1fr, 1fr, 1fr, 1fr),
    align: (left, center, center, center, center),
    stroke: none,
    table.hline(),
    table.header(
      [], table.cell(colspan: 2, align: center)[Apple M6], table.cell(colspan: 2, align: center)[i9-13900K],
    ),
    table.hline(start: 1, end: 3, stroke: 0.5pt), table.hline(start: 3, end: 5, stroke: 0.5pt),
    [Length], [LSH-256], [LSH-512], [LSH-256], [LSH-512],
    table.hline(stroke: 0.5pt),
    ..lengths.map(l => ([#size(l)], ..("m6", "i9").map(m => fams.map(fam => ratio-cell(m, fam, l))).flatten())).flatten(),
    table.hline(stroke: 0.5pt),
    [Geo. mean], ..("m6", "i9").map(m => fams.map(fam => [#r2(geo(m, fam))])).flatten(),
    table.hline(),
  ),
  caption: [Time of the fastest other implementation over shimeji's, for one message. Above 1, shimeji is faster. The letter names the fastest other implementation: #names.map(x => [#mark(x): #label.at(x)]).join(", "). Where shimeji is within #r0(100 * 0.05)% of it, the 95% bootstrap interval follows.],
) <tab:ratio>

On the M6, KISA's portable C is the fastest other implementation at every length.#for fam in fams { for l in lengths { assert(best("m6", fam, l).name == "kisa-ref") } } shimeji is #r2(calc.min(..lengths.map(l => best("m6", "lsh256", l).ratio))) to #r2(calc.max(..lengths.map(l => best("m6", "lsh256", l).ratio))) times as fast for LSH-256 and #r2(calc.min(..lengths.map(l => best("m6", "lsh512", l).ratio))) to #r2(calc.max(..lengths.map(l => best("m6", "lsh512", l).ratio))) times as fast for LSH-512. On long messages it takes #r2(cpb("m6", "lsh256", big, "shimeji")) and #r2(cpb("m6", "lsh512", big, "shimeji")) cycles per byte. shimeji's portable code alone runs at #r2(speedup("m6", "lsh256", big, "shimeji-portable", "kisa-ref")) and #r2(speedup("m6", "lsh512", big, "shimeji-portable", "kisa-ref")) times KISA's speed there, so the gain comes from the hybrid path. Bang et al. measured their single-message NEON cores at 0.75 to 0.90 times the speed of KISA's portable C compiled by clang on an M1 @bang2026batched. We compare with the same baseline, compiled by clang 21 on the M6, on long messages of LSH-256 and LSH-512. Against it, KISA's NEON code has speed ratios of #pair(speedup("m6", "lsh256", big, "kisa-neon", "kisa-ref"), speedup("m6", "lsh512", big, "kisa-neon", "kisa-ref")), go-krypto's #pair(speedup("m6", "lsh256", big, "go-krypto-neon", "kisa-ref"), speedup("m6", "lsh512", big, "go-krypto-neon", "kisa-ref")), and shimeji's hybrid path #pair(speedup("m6", "lsh256", big, "shimeji", "kisa-ref"), speedup("m6", "lsh512", big, "shimeji", "kisa-ref")).

On the i9, shimeji's LSH-256 is #r2(calc.min(..lengths.map(l => best("i9", "lsh256", l).ratio))) to #r2(calc.max(..lengths.map(l => best("i9", "lsh256", l).ratio))) times as fast as the fastest other implementation, which is KISA's SSSE3 code on short messages and Kim et al.’s on long ones.#for l in lengths { assert(best("i9", "lsh256", l).name == if l <= 64 { "kisa-ssse3" } else { "lsh-simd-ssse3" }) } The gap is widest at #size(widest), where shimeji is #r2(best("i9", "lsh256", widest).ratio) times as fast, and narrows to #r2(best("i9", "lsh256", big).ratio) at 1 MiB, where shimeji takes #r2(cpb("i9", "lsh256", big, "shimeji")) cycles per byte. For LSH-512 the four AVX2 kernels lie within a few percent of each other.#assert(hi("i9", "lsh512", avx2-512, cpb) / lo("i9", "lsh512", avx2-512, cpb) < 1.05) shimeji is #r2(best("i9", "lsh512", 64).ratio) times as fast as KISA's AVX2 code on 64-byte messages.#assert(best("i9", "lsh512", 64).name == "kisa-avx2") At 1 MiB it is #r2(best("i9", "lsh512", big).ratio) times as fast as go-krypto, the fastest other implementation there,#assert(best("i9", "lsh512", big).name == "go-krypto-avx2") and takes #r2(cpb("i9", "lsh512", big, "shimeji")) cycles per byte. #assert(slower("i9", "lsh512").len() > 0 and slower("i9", "lsh512").all(l => best("i9", "lsh512", l).name == "kisa-avx2"))KISA's AVX2 code is faster than shimeji at #slower("i9", "lsh512").map(size).join(" and "), where shimeji runs at #r2(worst("i9", "lsh512")) of its speed. Lee et al. moved the state into AVX-512 registers on an Intel Tiger Lake core and report 1.16 and 1.61 times the speed of KISA's AVX2 code for LSH-256 and LSH-512 on 4 KiB messages @lee2026avx512. The i9 has no AVX-512. At 4 KiB, shimeji's AVX2 code runs at #r2(speedup("i9", "lsh256", 4096, "shimeji", "kisa-avx2")) and #r2(speedup("i9", "lsh512", 4096, "shimeji", "kisa-avx2")) times the speed of the same KISA code. For LSH-256 the ratio includes the cost of the lane-crossing layout of KISA's AVX2 code, which its own SSSE3 code avoids. For LSH-512 the AVX2 kernels lie within #pct(hi("i9", "lsh512", avx2-512, cpb) / lo("i9", "lsh512", avx2-512, cpb) - 1) of each other on long messages, and Lee et al.’s larger gain comes with the AVX-512 registers that the i9 lacks.

== Footprint <footprint>

#let fp-rows = (
  ("m6", "shimeji, hybrid", "shimeji", "shimeji", "shimeji"),
  ("m6", "shimeji, portable", "shimeji", "shimeji-portable", "shimeji-portable"),
  ("m6", "KISA C", "kisa-ref", "kisa-ref", "kisa-ref"),
  ("m6", "KISA NEON", "kisa-neon", "kisa-neon", "kisa-neon"),
  ("i9", "shimeji, AVX2", "shimeji", "shimeji", "shimeji-avx2"),
  ("i9", "shimeji, SSSE3", "shimeji", "shimeji-ssse3", "shimeji-ssse3"),
  ("i9", "shimeji, portable", "shimeji", "shimeji-portable", "shimeji-portable"),
  ("i9", "KISA C", "kisa-ref", "kisa-ref", "kisa-ref"),
  ("i9", "KISA SSE2", "kisa-sse2", "kisa-sse2", "kisa-sse2"),
  ("i9", "KISA SSSE3", "kisa-ssse3", "kisa-ssse3", "kisa-ssse3"),
  ("i9", "KISA AVX2", "kisa-avx2", "kisa-avx2", "kisa-avx2"),
)
#let code(m, fam, imp) = if imp.starts-with("kisa") { kisa-code(m, fam, imp) } else { function(m, fam, imp).bytes }
#let kisa-stack = fp-rows.filter(r => r.at(2).starts-with("kisa"))

Neither shimeji nor KISA's code allocates memory, so a hash uses two kinds: the state a caller keeps between calls of a streaming hasher, and the stack of the call that hashes. @tab:footprint gives both, measured on the release builds of both machines, with the code bytes of each compression function. The stack is the peak of one call that hashes a 1000-byte message. We fill the stack of a fresh thread below its stack pointer with a known pattern and find the deepest word that the call overwrote.

#figure(
  table(
    columns: (auto, auto, auto, auto, auto, auto, auto),
    align: (left, right, right, right, right, right, right),
    stroke: none,
    table.hline(),
    table.header(
      [], table.cell(colspan: 2, align: center)[State (B)], table.cell(colspan: 2, align: center)[Stack (B)], table.cell(colspan: 2, align: center)[Code (B)],
    ),
    table.hline(start: 1, end: 3, stroke: 0.5pt), table.hline(start: 3, end: 5, stroke: 0.5pt), table.hline(start: 5, end: 7, stroke: 0.5pt),
    [], [256], [512], [256], [512], [256], [512],
    ..("m6", "i9").map(m => (
      table.hline(stroke: 0.5pt),
      table.cell(colspan: 7)[_#if m == "m6" [Apple M6] else [Intel Core i9-13900K]_],
      ..fp-rows.filter(r => r.at(0) == m).map(((m, title, st, sk, cd)) => (
        [#title],
        ..fams.map(fam => [#measured(m, "state", fam, st)]),
        ..fams.map(fam => [#measured(m, "stack", fam, sk)]),
        ..fams.map(fam => [#num(code(m, fam, cd))]),
      )).flatten(),
    )).flatten(),
    table.hline(),
  ),
  caption: [Bytes of state a streaming hasher keeps, peak stack of hashing one 1000-byte message, and code bytes of the compression function, for LSH-256 and LSH-512. shimeji's state is the same for every code path. KISA inlines its compression function into its `update` and `final` functions, and the table gives the sum of their code bytes.],
) <tab:footprint>

shimeji's streaming hasher keeps #measured("m6", "state", "lsh256", "shimeji") and #measured("m6", "state", "lsh512", "shimeji") bytes, as many as KISA's portable context, since both hold the chaining variable, one block and a length. KISA's SIMD contexts are #(measured("i9", "state", "lsh256", "kisa-avx2") - measured("i9", "state", "lsh256", "kisa-ref")) bytes larger, because of the alignment of their fields. One call of shimeji's `digest` takes #measured("m6", "stack", "lsh256", "shimeji") and #measured("m6", "stack", "lsh512", "shimeji") bytes of stack for LSH-256 and LSH-512 on the M6, and #measured("i9", "stack", "lsh256", "shimeji") and #measured("i9", "stack", "lsh512", "shimeji") on the i9. Every variant of KISA's code takes more on the same machine, at least #kisa-least-stack("m6", "lsh256") and #kisa-least-stack("m6", "lsh512") bytes on the M6 and #kisa-least-stack("i9", "lsh256") and #kisa-least-stack("i9", "lsh512") on the i9, because it keeps its whole context and its expanded sub-messages on the stack.#for m in ("m6", "i9") { for fam in fams { assert(measured(m, "stack", fam, "shimeji") < kisa-least-stack(m, fam)) } } The unrolled steps cost code size instead. shimeji's default compression functions take #num(code("m6", "lsh256", "shimeji")) and #num(code("m6", "lsh512", "shimeji")) bytes on the M6 and #num(code("i9", "lsh256", "shimeji-avx2")) and #num(code("i9", "lsh512", "shimeji-avx2")) on the i9. KISA's variants, which loop over pairs of steps, take #num(calc.min(..kisa-stack.map(r => calc.min(..fams.map(f => code(r.at(0), f, r.at(4))))))) to #num(calc.max(..kisa-stack.map(r => calc.max(..fams.map(f => code(r.at(0), f, r.at(4))))))) bytes.
