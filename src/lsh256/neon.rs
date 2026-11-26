//! As for LSH-512 (`lsh512/neon.rs`): the portable compression function
//! keeps the chaining variable in general-purpose registers and would spill
//! the 32-word message schedule; here it lives in four NEON registers per
//! sub-message, four words each, and reaches the scalar code through the
//! stack.

use core::arch::aarch64::*;

use super::{SubMessage, TAU};
use crate::neon::{load16, tbl};

#[derive(Clone, Copy)]
pub(super) struct Message([uint32x4_t; 4]);

const _: () = assert!(matches!(TAU, [_, _, _, _, 7, 4, 5, 6]));

const LOW: [u8; 16] = {
    let mut index = [0; 16];
    let mut i = 0;
    while i < 16 {
        index[i] = (4 * TAU[i / 4] + i % 4) as u8;
        i += 1;
    }
    index
};

impl Message {
    #[inline(always)]
    pub(super) fn load(block: &[u8]) -> [Self; 2] {
        // SAFETY: NEON is enabled (`neon.rs`).
        core::array::from_fn(|h| {
            Self(core::array::from_fn(|k| unsafe {
                vreinterpretq_u32_u8(load16(&block[64 * h + 16 * k..]))
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
            let low = |v| vreinterpretq_u32_u8(tbl(vreinterpretq_u8_u32(v), LOW));
            let tau = [
                low(o[0]),
                vextq_u32::<3>(o[1], o[1]),
                low(o[2]),
                vextq_u32::<3>(o[3], o[3]),
            ];
            Self(core::array::from_fn(|k| vaddq_u32(newer.0[k], tau[k])))
        }
    }

    /// Through memory, as for LSH-512: LLVM would otherwise forward the
    /// vector stores to the scalar reads as lane moves.
    #[inline(always)]
    fn words(&self) -> [u32; 16] {
        let mut words = [0; 16];
        for k in 0..4 {
            // SAFETY: NEON is enabled; `words` has 16 words, so 4 * k + 4 are
            // in bounds.
            unsafe { vst1q_u32(words[4 * k..].as_mut_ptr(), self.0[k]) };
        }
        *core::hint::black_box(&mut words)
    }
}
