= Introduction

LSH-256 and LSH-512 are the hash functions of the Korean standard KS X 3262 @kim2014lsh. Korean cryptographic designs use them where others use SHA-2: in PBKDF2 @choi2021pbkdf2 and in the hash-based signature schemes K-XMSS, K-SPHINCS#super[+] @sim2022kxmss and K-LMS @kang2026klms. A hash-based signature calls its hash function thousands of times per signature, on short inputs that do not depend on one another. The speed of LSH on short messages, and on many of them at once, therefore sets the speed of the scheme.

LSH has optimized implementations in three languages. KISA, the Korea Internet & Security Agency, distributes it in C @kisa2026lsh, Crypto++ in C++ @dai2026cryptopp and go-krypto in Go @ryuanerin2026gokrypto. All three carry hand-written SIMD code for x86, and the C and Go ones also for ARM. In Rust, LSH has only portable implementations, among them the allcrypt crate @allcrypt2026 and the lsh256 crate @lsh256rs2026.

This paper presents shimeji, an implementation of LSH-256 and LSH-512 written entirely in Rust, and measures it against these implementations on an Apple M6 and an Intel Core i9-13900K. shimeji has two code paths. The single-message path hashes one message at a time, as every existing implementation does. The batched path hashes several independent messages at once, one message per SIMD lane: four LSH-256 or two LSH-512 messages in a 128-bit NEON register, eight or four in a 256-bit AVX2 register.

Batching is an old technique for SHA-1 and SHA-2 @gueron2012simultaneous. For LSH it has been applied to LSH-256 on NEON @bang2026batched, and to pairs of LSH-256 messages held in the two halves of an AVX-512 register @lee2026avx512. On ARM, Bang et al. found that no single-message NEON code beats KISA's portable C, and that two LSH-512 messages per register gain nothing @bang2026batched. We contribute:

- a pure-Rust LSH-256 and LSH-512 with single-message and batched paths for NEON and AVX2, checked against KISA's code and the Crypto++ test vectors;
- a single-message path for ARM in which the integer units run the steps while NEON expands the message schedule, after the hybrid scalar and vector design that Becker and Kannwischer built for Keccak @becker2022hybrid;
- batched LSH on AVX2 and batched LSH-512 on x86, which no published implementation offers;
- a two-message LSH-512 batch for NEON that beats sequential C by rotating with the XAR instruction of the ARMv8.2 SHA-3 extension, which also absorbs the XOR of the step constant when the constants are rotated in advance;
- the first comparison of LSH implementations across C, C++, Go and Rust, on both machines, in cycles and instructions per byte, which explains where each of them spends its time.
