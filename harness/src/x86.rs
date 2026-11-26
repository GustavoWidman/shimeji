use crate::{Family, Implementation};

shimeji_hash!(ssse3_lsh256, ssse3_lsh512, digest_ssse3, unwrap);
shimeji_hash!(avx2_lsh256, avx2_lsh512, digest_avx2, unwrap);

fn ssse3() -> bool {
    is_x86_feature_detected!("ssse3")
}

fn avx2() -> bool {
    is_x86_feature_detected!("avx2")
}

macro_rules! entry {
    ($entry:ident, $name:literal, $family:ident, $hash:ident, $available:ident) => {
        pub(crate) const $entry: Implementation = Implementation {
            name: $name,
            family: Family::$family,
            digest_lengths: Family::$family.digest_lengths(),
            hash: $hash,
            available: $available,
            repeat: None,
        };
    };
}

entry!(SSSE3_256, "shimeji-ssse3", Lsh256, ssse3_lsh256, ssse3);
entry!(SSSE3_512, "shimeji-ssse3", Lsh512, ssse3_lsh512, ssse3);
entry!(AVX2_256, "shimeji-avx2", Lsh256, avx2_lsh256, avx2);
entry!(AVX2_512, "shimeji-avx2", Lsh512, avx2_lsh512, avx2);
