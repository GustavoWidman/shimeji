# one-message runs, 2026-11-26

the measurements the paper reports for hashing one message, taken at commit
94a504e with rustc 1.102.0-nightly (2026-11-12) and clang 21.1.8. each
`.tsv` has one row per repetition and a header with the host, compiler
versions and every flag; each `-summary.txt` is `harness summary` of it.

- `desktop-nixos.tsv`: Intel Core i9-13900K, NixOS. `shimeji-bench-mode on`
  (turbo off, every core at its 3.0 GHz base clock, performance governor,
  IRQs moved off CPUs 8-15), harness pinned to CPU 8, which the kernel keeps
  out of scheduling, timer ticks and RCU callbacks (`isolcpus`, `nohz_full`,
  `rcu_nocbs` on CPUs 8-15). command: `harness bench --cpu 8 --reps 30`.
  20,370 repetitions, all on CPU 8.
- `altar.tsv`: Apple M6, macOS. the thread runs at QoS user-interactive and
  records the core of every repetition; the summary keeps the repetitions on
  the two Super cores (6 and 7), 11,332 of 11,340. command: `harness bench
  --reps 30`, summary with `--cpus 6,7`.

cycles divided by nanoseconds gives the clock each repetition ran at. on
desktop-nixos it is 3.00 GHz for every implementation except go-krypto,
whose loop runs inside the Go runtime: at 16 to 4096 bytes its median falls
to 2.2 to 2.9 GHz, because the timed thread spends part of the loop off the
CPU, while the cycle counter counts only that thread. go-krypto's times
there are wall time; its cycle counts at those lengths understate its cost.
at 1 MiB its clock is 3.00 GHz.
