// Shared by main.typ and the sections: the measured numbers
// (paper/data/numbers.json, written by paper/analysis/analyze.py) and the
// helpers that print them.

#let n = json("data/numbers.json")

#let fix(x, d) = {
  let s = str(calc.round(x, digits: d))
  let parts = s.split(".")
  if d == 0 { parts.at(0) } else if parts.len() == 1 { s + "." + "0" * d } else {
    s + "0" * (d - parts.at(1).len())
  }
}
#let r0(x) = fix(x, 0)
#let r1(x) = fix(x, 1)
#let r2(x) = fix(x, 2)
#let num(k) = {
  let digits = str(int(calc.round(k))).clusters().rev()
  let out = ()
  for (i, c) in digits.enumerate() {
    if i > 0 and calc.rem(i, 3) == 0 { out.push(",") }
    out.push(c)
  }
  out.rev().join()
}
// A fraction as a whole percentage.
#let pct(x) = r0(100 * x) + "%"
#let times(x) = r2(x) + "×"
#let words = ("no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten")
#let count(k) = if k < words.len() { words.at(k) } else { str(k) }

#let lengths = (16, 64, 256, 1024, 4096, 16384, 1048576)
#let size(len) = if len >= 1048576 { str(int(len / 1048576)) + " MiB" } else if len >= 1024 {
  str(int(len / 1024)) + " KiB"
} else { str(len) + " B" }
#let block = (lsh256: 128, lsh512: 256)
#let steps = (lsh256: 26, lsh512: 28)

// Medians of one (machine, family, length, implementation) cell. Cycles on
// the i9 are time at its fixed clock, which the counters agree with for
// every implementation but go-krypto, whose thread is off the CPU for part
// of its loop; the M6 has no fixed clock, and its cycles are the counters'.
#let cell(m, fam, len, imp) = n.cells.at(m).at(fam).at(str(len)).at(imp)
#let ns(m, fam, len, imp) = cell(m, fam, len, imp).ns
#let cpb(m, fam, len, imp) = if m == "i9" {
  cell(m, fam, len, imp).ns * n.machines.i9.clock_ghz
} else { cell(m, fam, len, imp).cyc }
#let ipb(m, fam, len, imp) = cell(m, fam, len, imp).ins
#let ipc(m, fam, len, imp) = cell(m, fam, len, imp).ipc
// Time of `other` over time of `imp`: how many times faster `imp` is.
#let speedup(m, fam, len, imp, other) = ns(m, fam, len, other) / ns(m, fam, len, imp)
#let best(m, fam, len) = n.best.at(m).at(fam).at(str(len))
#let geo(m, fam) = n.geomean.at(m).at(fam)
// Per block and per step at 1 MiB, from the counters.
#let per(m, fam, imp) = n.derived.at(m).at(fam).at(imp)

// The release build's block functions (paper/analysis/footprint.py).
#let host = (m6: "m6", i9: "i9")
#let function(m, fam, imp) = n.footprint.at(m).functions.find(f => (
  f.family == fam and f.implementation == imp and "mnemonics" in f
))
#let mnem(m, fam, imp, ..names) = {
  let f = function(m, fam, imp)
  names.pos().map(k => f.mnemonics.at(k, default: 0)).sum()
}
#let insns(m, fam, imp) = function(m, fam, imp).mnemonics.values().sum()
#let measured(m, kind, fam, imp) = n.footprint.at(m).measured.find(r => (
  r.kind == kind and r.family == fam and r.implementation == imp
)).bytes
// Bytes of KISA's `update` and `final`, the two functions its compression
// function is inlined into.
#let kisa-code(m, fam, variant) = n.footprint.at(m).functions.filter(f => (
  f.family == fam and f.implementation == variant and f.at("function", default: "") in ("update", "final")
)).map(f => f.bytes).sum()

// The name the paper gives each implementation and code path.
#let label = (
  "shimeji": "shimeji",
  "shimeji-portable": "shimeji, portable",
  "shimeji-ssse3": "shimeji, SSSE3",
  "shimeji-avx2": "shimeji, AVX2",
  "kisa-ref": "KISA C",
  "kisa-neon": "KISA NEON",
  "kisa-sse2": "KISA SSE2",
  "kisa-ssse3": "KISA SSSE3",
  "kisa-avx2": "KISA AVX2",
  "lsh-simd-sse2": "Kim et al. SSE2",
  "lsh-simd-ssse3": "Kim et al. SSSE3",
  "lsh-simd-avx2": "Kim et al. AVX2",
  "pbkdf2-lsh-sse2": "Choi–Seo SSE2",
  "cryptopp-ref": "Crypto++",
  "cryptopp-avx2": "Crypto++ AVX2",
  "digestpp": "digestpp",
  "lsh256-rs": "lsh256 crate",
  "allcrypt": "allcrypt",
  "go-krypto-neon": "go-krypto NEON",
  "go-krypto-ssse3": "go-krypto SSSE3",
  "go-krypto-avx2": "go-krypto AVX2",
)
// "factors of 1.25 and 1.26", or "a factor of 1.24 for both" when the two
// print the same.
#let factors(a, b) = if r2(a) == r2(b) [a factor of #r2(a) for both] else [factors of #r2(a) and #r2(b)]
// "0.56 and 0.57", or "1.24 for both" when the two print the same.
#let pair(a, b) = if r2(a) == r2(b) [#r2(a) for both] else [#r2(a) and #r2(b)]
// The smallest peak stack of any KISA variant measured on machine `m`.
#let kisa-least-stack(m, fam) = calc.min(..n.footprint.at(m).measured.filter(r => (
  r.kind == "stack" and r.family == fam and r.implementation.starts-with("kisa")
)).map(r => r.bytes))
#let machine = (m6: "M6", i9: "i9")
#let family = (lsh256: "LSH-256", lsh512: "LSH-512")
