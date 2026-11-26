#import "/lib.typ": *

= Hashing one message <impl>

shimeji has one compression function, written in Rust and generic over the type of a sub-message, and runs it in four code paths (@tab:paths). The portable code runs on every target. On AArch64 the same function runs its steps in general-purpose registers and keeps its message schedule in NEON registers. On x86-64, two kernels written with intrinsics, one for SSSE3 and one for AVX2, keep the state in vector registers. This section takes each in turn: what limits the obvious code, what removes the limit, and what the step or the block then costs. Instruction counts per block come from the hardware counters on 1 MiB messages. Counts of individual instructions come from the disassembly of the release build.

#figure(
  table(
    columns: (auto, auto, auto, auto),
    align: (left, left, left, left),
    stroke: none,
    table.hline(),
    table.header([Code path], [Target], [State], [Message schedule]),
    table.hline(stroke: 0.5pt),
    [Portable], [any], [integer registers], [integer registers, stack],
    [Hybrid], [AArch64], [integer registers], [NEON registers],
    [SSSE3], [x86-64], [4 or 8 XMM], [XMM registers],
    [AVX2], [x86-64], [4 XMM or 4 YMM], [XMM or YMM registers],
    table.hline(),
  ),
  caption: [shimeji's code paths for one message, and the registers that hold the state of LSH-256 or LSH-512 and its message schedule. `digest` and the streaming hasher take the hybrid path on little-endian AArch64 and the fastest SIMD kernel the CPU supports on x86-64.],
) <tab:paths>

== The portable code <portable>

The compression function is fully unrolled. A macro writes out all 26 or 28 steps and the message expansion between them. The word permutation $sigma$ at the end of a step then only determines which variables the next step reads, so it compiles to no instruction. On AArch64 the portable LSH-512 block function has #mnem("m6", "lsh512", "shimeji-portable", "mov") register moves among its #num(insns("m6", "lsh512", "shimeji-portable")) instructions.

Each step XORs eight step constants into the state. x86-64 loads a 64-bit constant into a register with one `movabs`, but AArch64 needs four instructions, a `mov` and three `movk`, because one instruction carries 16 bits of immediate. Every constant of the unrolled function is known at compile time, so LLVM builds each one in place, which costs $4 dot 8 dot 28 = 896$ instructions per LSH-512 block. On AArch64, shimeji reads LSH-512’s constants through a pointer passed through `core::hint::black_box`, so LLVM cannot treat them as compile-time values. One `ldp` then loads two constants, 112 loads per block. LSH-256’s 32-bit constants take two instructions each and stay immediates.

The streaming hasher holds the chaining variable, a buffer of one block and the number of bytes in the buffer. `update` compresses whole blocks straight from the caller's slice and copies only a trailing partial block into the buffer. `finalize` copies the rest of the message into a zeroed block on its own stack, appends the `0x80` byte and compresses that block. The crate is `no_std` and allocates nothing.

== The integer and vector units together <hybrid>

#let p = "shimeji-portable"
#let h = "shimeji"
#let loads(imp) = mnem("m6", "lsh512", imp, "ldr", "ldp", "ldp.q")
#let stores(imp) = mnem("m6", "lsh512", imp, "str", "stp", "stp.q")
#let save(a, b) = 1 - b / a

On AArch64 the portable step keeps the sixteen words of the state and the temporaries of the mix function in general-purpose registers. There are 31 of them, too few to also hold the 32 words of the two sub-messages that the message expansion needs, so LLVM keeps the message schedule on the stack. The portable LSH-512 block function loads #loads(p) times and stores #stores(p) times per block. #num(16 * 27) of its #num(mnem("m6", "lsh512", p, "add")) additions expand the schedule, sixteen for each of the 27 sub-messages after the first two. The rotations cost little. AArch64’s `eor` can rotate its second operand within the same instruction. Of the three rotations of a column, only $beta$ needs an instruction of its own, because its result feeds an addition. Per block, that is #mnem("m6", "lsh512", p, "ror") `ror`. In all, portable LSH-512 retires #num(per("m6", "lsh512", p).ins_block) instructions per block at #r1(ipc("m6", "lsh512", 1048576, p)) per cycle on the M6, or #r1(per("m6", "lsh512", p).cyc_step) cycles per step (@tab:arm). KISA's C code retires more instructions at a higher rate and takes #r1(per("m6", "lsh512", "kisa-ref").cyc_step) cycles per step.

#figure(
  table(
    columns: (auto, auto, auto, auto, auto, auto, auto),
    align: (left, right, right, right, right, right, right),
    stroke: none,
    table.hline(),
    table.header(
      [], table.cell(colspan: 3, align: center)[LSH-256], table.cell(colspan: 3, align: center)[LSH-512],
    ),
    table.hline(start: 1, end: 4, stroke: 0.5pt), table.hline(start: 4, end: 7, stroke: 0.5pt),
    [], [Ins.], [IPC], [Cyc.], [Ins.], [IPC], [Cyc.],
    table.hline(stroke: 0.5pt),
    ..("kisa-ref", "shimeji-portable", "kisa-neon", "go-krypto-neon", "shimeji").map(imp => (
      [#label.at(imp)],
      ..("lsh256", "lsh512").map(fam => (
        [#num(per("m6", fam, imp).ins_block)],
        [#r1(ipc("m6", fam, 1048576, imp))],
        [#r1(per("m6", fam, imp).cyc_step)],
      )).flatten(),
    )).flatten(),
    table.hline(),
  ),
  caption: [Retired instructions per block (Ins.), instructions per cycle (IPC) and cycles per step (Cyc.) of one message of 1 MiB on the M6. KISA C and shimeji's portable code run on the integer units, KISA NEON and go-krypto NEON on NEON, and shimeji's hybrid path on both.],
) <tab:arm>

KISA's NEON code, go-krypto and every single-message core of Bang et al. @bang2026batched run the step in NEON. This cuts the instruction count, because one NEON instruction works on two words of LSH-512 or four of LSH-256. KISA's NEON code retires #pct(save(per("m6", "lsh512", "kisa-ref").ins_block, per("m6", "lsh512", "kisa-neon").ins_block)) fewer instructions per LSH-512 block than its C code. It retires them at #r1(ipc("m6", "lsh512", 1048576, "kisa-neon")) per cycle instead of #r1(ipc("m6", "lsh512", 1048576, "kisa-ref")), and takes #r1(per("m6", "lsh512", "kisa-neon").cyc_step) cycles per step. Two things make the NEON step latency-bound. NEON has no rotate instruction, so every rotation is a shift and a shift-and-insert, two dependent instructions on the path from one addition to the next. And the eight columns of a step fill only four vector registers of LSH-512 words, or two of LSH-256 words, so a step has four or two independent chains of dependent instructions where the scalar code has eight. Bang et al. found the same on an Apple M1, where none of their NEON cores beat KISA's portable C compiled by clang.

shimeji splits the work between the two units instead, after the hybrid Keccak of Becker and Kannwischer @becker2022hybrid. The integer units run the steps, and NEON holds and expands the message schedule, which the step only reads. A sub-message of LSH-512 takes eight vector registers of two words each, and a sub-message of LSH-256 takes four registers of four words. The two live sub-messages therefore occupy 16 or 8 of the 32 vector registers and never spill. Expanding an LSH-512 sub-message takes eight vector additions and #count(int(mnem("m6", "lsh512", h, "ext.16b") / 27)) `ext` instructions for $tau$. Expanding an LSH-256 sub-message takes four additions and #count(int(mnem("m6", "lsh256", h, "ext.16b", "rev64.4s") / 25)) `ext` and `rev64` instructions. The step takes each sub-message through the stack: four stores of register pairs and eight `ldp` of word pairs per step. The code passes the buffer through `black_box`. Without it, LLVM forwards the stored vectors to the scalar reads as sixteen lane moves (`fmov`, `mov`), which cost more than the twelve memory instructions.

Per LSH-512 block the hybrid path executes #mnem("m6", "lsh512", h, "add") integer additions instead of #num(mnem("m6", "lsh512", p, "add")), #loads(h) loads instead of #loads(p), and #stores(h) stores instead of #stores(p). NEON executes #mnem("m6", "lsh512", h, "add.2d") vector additions and #mnem("m6", "lsh512", h, "ext.16b") `ext`. The block shrinks from #num(per("m6", "lsh512", p).ins_block) to #num(per("m6", "lsh512", h).ins_block) retired instructions, #pct(save(per("m6", "lsh512", p).ins_block, per("m6", "lsh512", h).ins_block)) fewer. The rate rises from #r1(ipc("m6", "lsh512", 1048576, p)) to #r1(ipc("m6", "lsh512", 1048576, h)) instructions per cycle, because the vector instructions run on units that the scalar step leaves idle. A step takes #r1(per("m6", "lsh512", h).cyc_step) cycles instead of #r1(per("m6", "lsh512", p).cyc_step). For LSH-256 the block goes from #num(per("m6", "lsh256", p).ins_block) to #num(per("m6", "lsh256", h).ins_block) instructions and the step from #r1(per("m6", "lsh256", p).cyc_step) to #r1(per("m6", "lsh256", h).cyc_step) cycles. Bang et al. left the hybrid design out of their study @bang2026batched. On the M6 it is the fastest way to hash one LSH message.

== SSSE3 and AVX2 <x86>

#let ssse3 = "shimeji-ssse3"
#let avx2 = "shimeji-avx2"

*LSH-256.* The sixteen 32-bit words of the state fit in four 128-bit registers: $p$ holds words 0 to 3, $q$ words 4 to 7, $r$ words 8 to 11 and $s$ words 12 to 15. A step mixes word $l$ with word $l + 8$, so its arithmetic works lane by lane on $p$ with $r$ and on $q$ with $s$. The permutation $sigma$ moves whole registers, $(p, q, r, s) <- (q, s, p, r)$. It also reorders the four words inside each register, by one fixed order $F$ in the new $p$ and $r$ and by another, $G$, in the new $q$ and $s$. KISA's SSSE3 code restores canonical order after every step with four `pshufd`, which sit on the step's critical path. shimeji never restores it. All four registers share one lane order, which advances by $F$ each step. Because $F$ has order three, the lane order repeats every three steps. The new $q$ and $s$ come from the $Y$ outputs of the mix function, which the step rotates by $gamma$ anyway. Since $gamma$ moves whole bytes, one `pshufb` per register rotates every word and moves it into the next step's order in the same instruction. The step constants are stored in each step's lane order. One `pshufd` per register puts the message words into that order, off the critical path, except on every third step, when the order is canonical. The critical path of a step thus has no shuffle beyond the two `pshufb` that $gamma$ needs in any layout. The AVX2 block function has #mnem("i9", "lsh256", avx2, "vpshufb") `vpshufb`, two per step, and #mnem("i9", "lsh256", avx2, "vpshufd") `vpshufd`. All but four of the `vpshufd` work on the message schedule, and those four return the state to canonical order at the end of the block.

The AVX2 build of LSH-256 is the same code in VEX encoding. Its three-operand instructions save the register copies that SSE's two-operand forms need. The SSSE3 build has #mnem("i9", "lsh256", ssse3, "movdqa") `movdqa` per block and the AVX2 build has #mnem("i9", "lsh256", avx2, "vmovdqa") `vmovdqa`, and the block shrinks from #num(insns("i9", "lsh256", ssse3)) to #num(insns("i9", "lsh256", avx2)) instructions. It keeps 128-bit registers on purpose. In 256-bit registers, with $p$ and $q$ in one and $r$ and $s$ in the other, $sigma$ needs two lane-crossing permutes per step on the critical path, and a lane-crossing permute takes three cycles on Intel cores @abel2019uops. KISA's and Kim et al.’s AVX2 code are built that way, and @results shows what it costs them.

*LSH-512.* Sixteen 64-bit words fill four 256-bit registers, $p$, $q$, $r$ and $s$, with the same assignment of words and the same structure of $sigma$. Here a word moves between the two 128-bit halves of a register only through `vpermq`, which takes three cycles and runs on a single port @abel2019uops. KISA's AVX2 code applies four `vpermq` to the state in every step, 112 per block, all on the critical path. Some crossing cannot be avoided. The pairs of words that three consecutive steps mix connect all sixteen words, so in every three steps at least one must move words across halves. shimeji's even steps move none. They start in a lane order in which words 1 and 2 of each register are swapped. The new $p$ and $r$ take the order that $sigma$ leaves them in, and the new $q$ and $s$ end up in the even order again through the `vpshufb` of $gamma$, which reorders words within halves. The odd steps return $p$, $r$ and $s$ to the even order with three `vpermq`, 42 per block. The message expansion works in the even order as well. The $q$ and $s$ quarters of a sub-message are then always in the step's order, and an odd step reorders only the $p$ and $r$ quarters of its sub-message. With four `vpermq` for $tau$ in each expansion and eight that put each block in order as it is loaded, the block function has #mnem("i9", "lsh512", avx2, "vpermq") `vpermq`. Eight of them convert the chaining variable on entry and exit. One empty `asm!` statement takes the next sub-message as an input and passes $p$'s $alpha$ rotation through. It emits no instruction, but it changes the order in which LLVM schedules the instructions around the boundary between steps. Without it, the kernel takes #r1(100 * (n.barrier.at("1048576").ratio - 1))% more cycles on 1 MiB messages and #r1(100 * (n.barrier.at("64").ratio - 1))% more on 64-byte ones on the i9, while retiring #r1(100 * (1 - n.barrier.at("1048576").ins_without / n.barrier.at("1048576").ins_with))% fewer instructions. Placed on $q$'s rotation instead, the same statement gives no speedup, so the speedup comes from that instruction order and not from expanding the sub-message early.

The SSSE3 kernel of LSH-512 keeps the state in eight 128-bit registers of two words each, in canonical order. It moves words between the two registers of a quarter with two-source shuffles (`punpcklqdq`, `punpckhqdq`, `palignr`), and the `pshufb` of $gamma$ undoes the word swap that these shuffles leave.

*Choosing a kernel.* The crate is `no_std`, so it cannot use the standard library's feature detection. On first use it reads `cpuid` for SSSE3 (leaf 1) and AVX2 (leaf 7) and checks with `cpuid` and `xgetbv` that the operating system saves the 256-bit registers. It caches the answer in an atomic byte. Each kernel is a function compiled with its target feature, behind a safe wrapper that checks the feature first. The hasher calls the fastest kernel through a function pointer for every run of whole blocks. A build for a CPU that has AVX2 (`-C target-cpu=native`) skips the check.

== Correctness <correctness>

The known-answer tests check every LSH test vector of Crypto++, #n.kat.total of them for five of the six digests. Crypto++ has none for LSH-512-224. The tests hash each message whole, in one call to `update`, and in eight sequences of calls cut at seeded random offsets. About half of the cuts fall within two bytes of a block boundary, so block boundaries fall both inside a call to `update` and between two calls. A differential test hashes every length from 0 to 3000 bytes and four lengths around 64 KiB and 1 MiB. The lengths up to 3000 bytes cover every length within two bytes of a multiple of either block size, up to four blocks. The test hashes each length with all six digests and every implementation of @tab:impls that the machine runs, starting the messages at varying offsets. It requires every digest to equal the digest of shimeji's portable code. On LSH-512-224 it is the only check. It runs on both machines, so it checks the hybrid path, both x86 kernels and the portable code against each other and against every other implementation in @tab:impls.
