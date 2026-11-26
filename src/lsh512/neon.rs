//! The portable compression function keeps the chaining variable and the
//! step's temporaries in general-purpose registers, which leaves too few of
//! them for the 32-word message schedule: compiled alone, it spills the
//! schedule to the stack and spends 16 of its about 80 integer operations per
//! step expanding it. Here the schedule lives in eight NEON registers per
//! sub-message, two words each, and the otherwise idle vector units expand
//! it; each sub-message reaches the scalar code through the stack.

use core::arch::aarch64::*;

use super::{SubMessage, TAU};
use crate::neon::load16;

#[derive(Clone, Copy)]
pub(super) struct Message([uint64x2_t; 8]);

const _: () = assert!(matches!(TAU, [3, 2, 0, 1, 7, 4, 5, 6]));

impl Message {
    #[inline(always)]
    pub(super) fn load(block: &[u8]) -> [Self; 2] {
        // SAFETY: NEON is enabled (`neon.rs`).
        core::array::from_fn(|h| {
            Self(core::array::from_fn(|k| unsafe {
                vreinterpretq_u64_u8(load16(&block[128 * h + 16 * k..]))
            }))
        })
    }
}

impl SubMessage for Message {
    #[inline(always)]
    fn expand(older: Self, newer: &Self) -> Self {
        let o = older.0;
        // SAFETY: NEON is enabled (`neon.rs`).
        unsafe {
            let tau = [
                vextq_u64::<1>(o[1], o[1]),
                o[0],
                vextq_u64::<1>(o[3], o[2]),
                vextq_u64::<1>(o[2], o[3]),
                vextq_u64::<1>(o[5], o[5]),
                o[4],
                vextq_u64::<1>(o[7], o[6]),
                vextq_u64::<1>(o[6], o[7]),
            ];
            Self(core::array::from_fn(|k| vaddq_u64(newer.0[k], tau[k])))
        }
    }

    /// Through memory: LLVM would otherwise forward the vector stores to
    /// the scalar reads as 16 lane moves (`fmov`, `mov`), which cost more
    /// than eight `stp` and eight `ldp`.
    #[inline(always)]
    fn words(&self) -> [u64; 16] {
        let mut words = [0; 16];
        for k in 0..8 {
            // SAFETY: NEON is enabled; `words` has 16 words, so 2 * k + 2 are
            // in bounds.
            unsafe { vst1q_u64(words[2 * k..].as_mut_ptr(), self.0[k]) };
        }
        *core::hint::black_box(&mut words)
    }
}
