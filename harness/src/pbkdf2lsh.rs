use crate::{Family, Implementation};

unsafe extern "C" {
    fn pbkdf2_lsh_sse2_lsh256_repeat(msg: *const u8, msg_len: u32, output: *mut u8, iters: u64);
}

fn call(message: &[u8], out: &mut [u8], iters: u64) {
    assert_eq!(
        out.len(),
        32,
        "Choi and Seo's code computes LSH-256-256 only"
    );
    let len = u32::try_from(message.len()).expect("message length fits in 32 bits");
    // SAFETY: the code reads `len` bytes from `message` and writes 32 bytes.
    unsafe { pbkdf2_lsh_sse2_lsh256_repeat(message.as_ptr(), len, out.as_mut_ptr(), iters) }
}

fn hash(message: &[u8], out: &mut [u8]) {
    call(message, out, 1)
}

fn repeat(message: &[u8], out: &mut [u8], iters: u64) {
    call(message, out, iters)
}

pub(crate) const SSE2_256: Implementation = Implementation {
    name: "pbkdf2-lsh-sse2",
    family: Family::Lsh256,
    digest_lengths: &[32],
    hash,
    repeat: Some(repeat),
    available: || is_x86_feature_detected!("sse2"),
};
