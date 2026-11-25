#!/usr/bin/env bash
# Fetches the reference material this crate is tested against, pinned to
# fixed commits, and checks it against refs/SHA256SUMS.
#
#   refs/kisa/<lsh256|lsh512>/<variant>/  KISA's LSH code as packaged in SUPERCOP
#   refs/kat/                              Crypto++ LSH test vectors
#   refs/lsh-simd/<sse2|ssse3|avx2>/       Kim, Jung, Ju and Song's SIMD LSH code
#
# Run with --update to rewrite SHA256SUMS after changing a pin.
set -euo pipefail

supercop=200cd30ca1f501463bd5cdd3da115a20d688d8fb
cryptopp=5873a73a231c95771f5bbf3466a484952fb22780
lshsimd=7bf958ff72e2bc0bb1482b4b29bd22370cd99c9c

refs=$(cd "$(dirname "$0")" && pwd)
cd "$refs"

fetch() {
  mkdir -p "$(dirname "$2")"
  curl -fsSL --retry 3 -o "$2" "$1"
}

for alg in lsh256 lsh512; do
  for variant in ref sse2 ssse3 avx2 neon; do
    for file in lsh-impl.c lsh-impl.h lsh.c lsh.h lsh_def.h lsh_local.h; do
      fetch "https://raw.githubusercontent.com/jedisct1/supercop/$supercop/crypto_hash/$alg/$variant/$file" \
        "kisa/$alg/$variant/$file"
    done
  done
done

for file in lsh256.txt lsh512.txt lsh512_256.txt; do
  fetch "https://raw.githubusercontent.com/weidai11/cryptopp/$cryptopp/TestVectors/$file" "kat/$file"
done

# The XOP variant is left out: it needs AMD XOP hardware. The benchmark
# drivers and print helpers next to these files are not needed.
lshsimd_url=https://raw.githubusercontent.com/JuYoungjin/LSH-SIMD-Implementation/$lshsimd
fetch "$lshsimd_url/LICENSE" lsh-simd/LICENSE
for variant in SSE2:SSE SSSE3:SSE AVX2:AVX2; do
  dir=${variant%%:*}
  header=${variant##*:}
  local_dir=lsh-simd/$(tr '[:upper:]' '[:lower:]' <<<"$dir")
  for file in lsh_common.h \
    "lsh256_ref_${header}_OPT.h" "lsh256_ref_${dir}_OPT.c" \
    "lsh512_ref_${header}_OPT.h" "lsh512_ref_${dir}_OPT.c"; do
    fetch "$lshsimd_url/$dir/$file" "$local_dir/$file"
  done
done

if [[ ${1:-} == --update ]]; then
  find kisa kat lsh-simd -type f | LC_ALL=C sort | xargs shasum -a 256 > SHA256SUMS
else
  shasum -a 256 --quiet -c SHA256SUMS
fi
