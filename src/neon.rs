//! `lib.rs` compiles this module only when the `neon` target feature is
//! enabled for the whole build, as it is on every standard aarch64 target.
//! That is what every `unsafe` block below relies on: they call NEON
//! intrinsics, which are unsafe only because they require that feature, and
//! nothing else unless their own comment says so.

use core::arch::aarch64::*;

#[inline(always)]
pub(crate) fn load16(bytes: &[u8]) -> uint8x16_t {
    let bytes: &[u8; 16] = bytes[..16].try_into().unwrap();
    // SAFETY: `bytes` is 16 readable bytes; `ld1` has no alignment requirement.
    unsafe { vld1q_u8(bytes.as_ptr()) }
}

#[inline(always)]
pub(crate) fn tbl(v: uint8x16_t, index: [u8; 16]) -> uint8x16_t {
    // SAFETY: NEON is enabled (module docs).
    unsafe { vqtbl1q_u8(v, load16(&index)) }
}
