use crate::{Family, Implementation};

type Export = unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32;

unsafe extern "C" {
    fn digestpp_lsh256(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
    fn digestpp_lsh512(msg: *const u8, msglen: usize, out: *mut u8, outlen: usize) -> i32;
}

fn call(export: Export, message: &[u8], out: &mut [u8]) {
    // SAFETY: digestpp reads `message.len()` bytes and writes `out.len()`.
    let err = unsafe { export(message.as_ptr(), message.len(), out.as_mut_ptr(), out.len()) };
    assert_eq!(err, 0, "digestpp has no {}-byte digest", out.len());
}

fn lsh256(message: &[u8], out: &mut [u8]) {
    call(digestpp_lsh256, message, out)
}

fn lsh512(message: &[u8], out: &mut [u8]) {
    call(digestpp_lsh512, message, out)
}

pub(crate) const LSH256: Implementation = Implementation {
    name: "digestpp",
    family: Family::Lsh256,
    digest_lengths: Family::Lsh256.digest_lengths(),
    hash: lsh256,
    repeat: None,
    available: || true,
};

pub(crate) const LSH512: Implementation = Implementation {
    name: "digestpp",
    family: Family::Lsh512,
    digest_lengths: Family::Lsh512.digest_lengths(),
    hash: lsh512,
    repeat: None,
    available: || true,
};
