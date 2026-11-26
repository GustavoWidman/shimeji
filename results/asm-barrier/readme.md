# asm-barrier runs, 2026-11-26

what the empty `asm!` in `after` (`src/lsh512/x86.rs`) is worth: the one-
message LSH-512 AVX2 kernel with it (`with-*.tsv`) and without it
(`without-*.tsv`, the same source with `no-barrier.patch` applied, so their
headers mark commit 94a504e dirty), LSH-512-512 at 64, 1024 and 1048576
bytes on the Intel Core i9-13900K, in the bench mode and on the CPU of the
one-message runs (`../single/readme.md`).

each file is one `harness bench --impl shimeji-avx2 --digest lsh512-512
--len 64,1024,1048576 --reps 21 --cpu 8` run; the two builds ran
alternately, with, without, without, with, with, without, without, with
(files 1 to 4).
