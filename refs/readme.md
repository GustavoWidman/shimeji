# refs

reference material, fetched by `fetch.sh` at pinned commits and checked
against `SHA256SUMS`. nothing here is compiled into the crate.

- `kisa/` is KISA's LSH code (https://seed.kisa.or.kr), as packaged in
  SUPERCOP (mirror: https://github.com/jedisct1/supercop). each of `lsh256`
  and `lsh512` comes in five variants: portable C (`ref`), SSE2, SSSE3, AVX2
  and NEON. SUPERCOP's sixth, XOP, needs AMD XOP hardware and is not
  fetched. the code is copyright 2016 NSR (National Security Research
  Institute) under the MIT license, whose notice heads every file.
- `kat/` holds the LSH test vectors from Crypto++
  (https://github.com/weidai11/cryptopp, `TestVectors/`), copied from the
  commit `flake.nix` pins: LSH-256-224,
  LSH-256-256, LSH-512-384, LSH-512-512 and LSH-512-256. Crypto++ places
  its individual files in the public domain.
- `lsh-simd/` is the code of Kim, Jung, Ju and Song, "Fast Implementation
  of LSH With SIMD", IEEE Access 7 (2019)
  (https://github.com/JuYoungjin/LSH-SIMD-Implementation): LSH-256 and
  LSH-512 with SSE2, SSSE3 and AVX2, one directory each, keeping the
  upstream file names. its XOP variant, which needs AMD XOP hardware, and
  its benchmark drivers are not fetched. the code is copyright 2019
  JuYoungjin under the MIT license in `lsh-simd/LICENSE`.
- `pbkdf2-lsh/` is not in the repository. `harness/build.rs` downloads Choi
  and Seo's SSE2 LSH-256 there from
  https://github.com/ondoli0312/PBKDF2-HMAC-LSH256 at a pinned commit and
  checks it against pinned SHA-256 digests; that repository has no license,
  so its code is fetched for each build instead of kept here.
