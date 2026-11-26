use shimeji::{Lsh256_224, Lsh256_256, Lsh512_256, Lsh512_384, Lsh512_512};

struct Vector {
    name: String,
    message: Vec<u8>,
    digest: Vec<u8>,
}

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.split_whitespace().collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// Parses a message field: hex, optionally split by spaces, where a token
/// `r<count>` repeats the token after it `count` times.
fn message(field: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut repeat = 1;
    for token in field.split_whitespace() {
        match token.strip_prefix('r') {
            Some(count) => repeat = count.parse().unwrap(),
            None => {
                out.extend(hex(token).repeat(repeat));
                repeat = 1;
            }
        }
    }
    out
}

fn vectors(file: &str) -> Vec<Vector> {
    let path = format!("{}/refs/kat/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).unwrap();
    let mut name = String::new();
    let mut pending = None;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("Name:") {
            name = v.trim().to_owned();
        } else if let Some(v) = line.strip_prefix("Message:") {
            pending = Some(message(v));
        } else if let Some(v) = line.strip_prefix("Digest:") {
            out.push(Vector {
                name: name.clone(),
                message: pending.take().unwrap(),
                digest: hex(v),
            });
        }
    }
    out
}

macro_rules! chunked {
    ($hasher:ty) => {
        |message: &[u8], chunk: usize| {
            let mut hasher = <$hasher>::new();
            for part in message.chunks(chunk) {
                hasher.update(part);
            }
            hasher.finalize().to_vec()
        }
    };
}

fn hasher(name: &str) -> fn(&[u8], usize) -> Vec<u8> {
    match name {
        "LSH-224" => chunked!(Lsh256_224),
        "LSH-256" => chunked!(Lsh256_256),
        "LSH-384" => chunked!(Lsh512_384),
        "LSH-512" => chunked!(Lsh512_512),
        "LSH-512-256" => chunked!(Lsh512_256),
        other => panic!("no hasher for {other}"),
    }
}

fn check(file: &str, expected: usize) {
    let vectors = vectors(file);
    assert_eq!(vectors.len(), expected, "{file}: vector count");
    for v in &vectors {
        let hash = hasher(&v.name);
        for chunk in [usize::MAX, 1, 3, 127, 128, 129, 255, 256, 257] {
            assert_eq!(
                hash(&v.message, chunk),
                v.digest,
                "{} on {} bytes in chunks of {chunk}",
                v.name,
                v.message.len()
            );
        }
    }
}

#[test]
fn lsh256() {
    check("lsh256.txt", 294);
}

#[test]
fn lsh512() {
    check("lsh512.txt", 294);
}

#[test]
fn lsh512_256() {
    check("lsh512_256.txt", 147);
}
