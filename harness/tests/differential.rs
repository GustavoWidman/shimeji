use harness::{Digest, Family, Implementation, SplitMix64, implementations};

const LONG: [usize; 4] = [65_535, 65_536, 65_537, 1 << 20];

fn lengths() -> impl Iterator<Item = usize> {
    (0..=3000).chain(LONG)
}

fn portable(family: Family) -> &'static Implementation {
    implementations()
        .find(|imp| imp.name == "shimeji-portable" && imp.family == family)
        .unwrap()
}

#[test]
fn every_digest_has_an_independent_implementation() {
    for digest in Digest::all() {
        assert!(
            implementations()
                .any(|imp| !imp.name.starts_with("shimeji") && imp.digests().any(|d| d == digest)),
            "nothing to compare {} against",
            digest.name()
        );
    }
}

#[test]
fn every_implementation_matches_portable() {
    let mut rng = SplitMix64::new(0x5348_494d_454a_4931);
    let mut buf = vec![0u8; (1 << 20) + 64];
    let mut mismatches = Vec::new();
    let mut compared = 0usize;
    for len in lengths() {
        let offset = rng.below(64);
        let message = &mut buf[offset..offset + len];
        rng.fill(message);
        let message = &*message;
        for imp in implementations().filter(|imp| imp.name != "shimeji-portable") {
            let reference = portable(imp.family);
            for digest in imp.digests() {
                let mut want = [0u8; 64];
                let mut got = [0u8; 64];
                (reference.hash)(message, &mut want[..digest.bytes]);
                (imp.hash)(message, &mut got[..digest.bytes]);
                compared += 1;
                if want != got {
                    mismatches.push(format!("{} {} length {len}", imp.name, digest.name()));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {compared} digests differ, first: {:?}",
        mismatches.len(),
        &mismatches[..mismatches.len().min(10)]
    );
    eprintln!("{compared} digests compared");
}
