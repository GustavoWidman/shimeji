//! The AVX2 build is the same code with VEX encoding, whose three operands
//! save the register copies of the two-operand SSE forms. It keeps 128-bit
//! registers: in 256-bit registers (`[p | q]`, `[r | s]`) sigma needs two
//! lane-crossing permutes per step, three cycles each on the step's critical
//! path, and that is slower than two 128-bit chains side by side.

use core::arch::x86_64::*;

use super::{ALPHA, BETA, BLOCK, Blocks, GAMMA, SC, SIGMA, STEPS, TAU};

const fn quarter_map(dst: usize, src: usize) -> [usize; 4] {
    let mut map = [0; 4];
    let mut i = 0;
    while i < 4 {
        map[i] = SIGMA[4 * dst + i] - 4 * src;
        i += 1;
    }
    map
}

const SIGMA_X: [usize; 4] = quarter_map(0, 1);
const SIGMA_Y: [usize; 4] = quarter_map(1, 3);

const fn position(map: &[usize; 4], word: usize) -> usize {
    let mut i = 0;
    while map[i] != word {
        i += 1;
    }
    i
}

const ORDER: [[usize; 4]; STEPS + 1] = {
    assert!(matches!(quarter_map(2, 0), SIGMA_X) && matches!(quarter_map(3, 2), SIGMA_Y));
    let mut order = [[0; 4]; STEPS + 1];
    order[0] = [0, 1, 2, 3];
    let mut j = 0;
    while j < STEPS {
        let mut l = 0;
        while l < 4 {
            order[j + 1][l] = position(&SIGMA_X, order[j][l]);
            l += 1;
        }
        j += 1;
    }
    assert!(
        matches!(order[3], [0, 1, 2, 3]),
        "the order repeats every three steps"
    );
    order
};

#[repr(align(32))]
struct Aligned<T>(T);

static SC_LANES: Aligned<[[u32; 8]; STEPS]> = Aligned({
    let mut sc = [[0; 8]; STEPS];
    let mut j = 0;
    while j < STEPS {
        let mut l = 0;
        while l < 4 {
            sc[j][l] = SC[j][ORDER[j][l]];
            sc[j][4 + l] = SC[j][4 + ORDER[j][l]];
            l += 1;
        }
        j += 1;
    }
    sc
});

static GAMMA_SHUFFLE: Aligned<[[[u8; 16]; 2]; 3]> = Aligned({
    let mut index = [[[0; 16]; 2]; 3];
    let mut p = 0;
    while p < 3 {
        let mut which = 0;
        while which < 2 {
            let gamma_base = if which == 0 { 4 } else { 0 };
            let mut l = 0;
            while l < 4 {
                let old = SIGMA_Y[ORDER[p + 1][l]];
                let from = position(&ORDER[p], old);
                let bytes = GAMMA[gamma_base + old] as usize / 8;
                let mut b = 0;
                while b < 4 {
                    index[p][which][4 * l + b] = (4 * from + (b + 4 - bytes) % 4) as u8;
                    b += 1;
                }
                l += 1;
            }
            which += 1;
        }
        p += 1;
    }
    index
});

const fn shuffle_imm(order: [usize; 4]) -> i32 {
    (order[0] | order[1] << 2 | order[2] << 4 | order[3] << 6) as i32
}

const fn unshuffle_imm(order: [usize; 4]) -> i32 {
    shuffle_imm([
        position(&order, 0),
        position(&order, 1),
        position(&order, 2),
        position(&order, 3),
    ])
}

const TAU_LOW: i32 = shuffle_imm([TAU[0], TAU[1], TAU[2], TAU[3]]);
const TAU_HIGH: i32 = shuffle_imm([TAU[4] - 4, TAU[5] - 4, TAU[6] - 4, TAU[7] - 4]);

type SubMessage = [__m128i; 4];

macro_rules! reorder {
    ($shuffle:ident, $m:expr, $j:expr) => {
        match $j % 3 {
            0 => $m,
            1 => $shuffle::<{ shuffle_imm(ORDER[1]) }>($m),
            _ => $shuffle::<{ shuffle_imm(ORDER[2]) }>($m),
        }
    };
}

#[inline(always)]
fn load_128(bytes: &[u8]) -> __m128i {
    let bytes: &[u8; 16] = bytes[..16].try_into().unwrap();
    // SAFETY: 16 readable bytes; `loadu` has no alignment requirement, and
    // SSE2 is part of x86-64.
    unsafe { _mm_loadu_si128(bytes.as_ptr().cast()) }
}

struct Message([SubMessage; 2]);

impl Message {
    #[inline]
    #[target_feature(enable = "ssse3")]
    fn new(block: &[u8]) -> Self {
        Self([
            core::array::from_fn(|k| load_128(&block[16 * k..])),
            core::array::from_fn(|k| load_128(&block[64 + 16 * k..])),
        ])
    }

    #[inline]
    #[target_feature(enable = "ssse3")]
    fn next(&mut self, j: usize) -> SubMessage {
        let m = &mut self.0;
        if j > 1 {
            let (older, newer) = (m[j % 2], m[(j + 1) % 2]);
            m[j % 2] = [
                _mm_add_epi32(newer[0], _mm_shuffle_epi32::<TAU_LOW>(older[0])),
                _mm_add_epi32(newer[1], _mm_shuffle_epi32::<TAU_HIGH>(older[1])),
                _mm_add_epi32(newer[2], _mm_shuffle_epi32::<TAU_LOW>(older[2])),
                _mm_add_epi32(newer[3], _mm_shuffle_epi32::<TAU_HIGH>(older[3])),
            ];
        }
        match j {
            STEPS => m[j % 2],
            _ => m[j % 2].map(|m| reorder!(_mm_shuffle_epi32, m, j)),
        }
    }
}

macro_rules! rotl {
    ($x:expr, $n:expr) => {{
        let x = $x;
        _mm_or_si128(
            _mm_slli_epi32::<{ $n as i32 }>(x),
            _mm_srli_epi32::<{ 32 - $n as i32 }>(x),
        )
    }};
}

macro_rules! step {
    ($v:expr, $m:expr, $j:expr) => {{
        let [p, q, r, s] = $v;
        let m: SubMessage = $m;
        // SAFETY: the constants and the shuffle indices are aligned 16-byte
        // rows of statics.
        let (cp, cq, to_q, to_s) = unsafe {
            (
                _mm_load_si128(SC_LANES.0[$j].as_ptr().cast()),
                _mm_load_si128(SC_LANES.0[$j][4..].as_ptr().cast()),
                _mm_load_si128(GAMMA_SHUFFLE.0[$j % 3][0].as_ptr().cast()),
                _mm_load_si128(GAMMA_SHUFFLE.0[$j % 3][1].as_ptr().cast()),
            )
        };
        let (mut xp, mut xq) = (_mm_xor_si128(p, m[0]), _mm_xor_si128(q, m[1]));
        let (mut yr, mut ys) = (_mm_xor_si128(r, m[2]), _mm_xor_si128(s, m[3]));
        xp = _mm_add_epi32(xp, yr);
        xq = _mm_add_epi32(xq, ys);
        xp = _mm_xor_si128(rotl!(xp, ALPHA[$j % 2]), cp);
        xq = _mm_xor_si128(rotl!(xq, ALPHA[$j % 2]), cq);
        yr = rotl!(_mm_add_epi32(yr, xp), BETA[$j % 2]);
        ys = rotl!(_mm_add_epi32(ys, xq), BETA[$j % 2]);
        xp = _mm_add_epi32(xp, yr);
        xq = _mm_add_epi32(xq, ys);
        [
            xq,
            _mm_shuffle_epi8(ys, to_q),
            xp,
            _mm_shuffle_epi8(yr, to_s),
        ]
    }};
}

macro_rules! steps {
    ($v:ident, $m:ident; $($j:literal)+) => { $( $v = step!($v, $m.next($j), $j); )+ };
}

macro_rules! compress_blocks {
    ($cv:expr, $blocks:expr) => {{
        let cv: &mut [u32; 16] = $cv;
        // SAFETY: `cv` has 16 words, so 4 * k + 4 words are in bounds.
        let mut v: [__m128i; 4] =
            core::array::from_fn(|k| unsafe { _mm_loadu_si128(cv[4 * k..].as_ptr().cast()) });
        for block in $blocks.as_chunks::<BLOCK>().0 {
            let mut m = Message::new(block);
            steps!(v, m; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25);
            const BACK: i32 = unshuffle_imm(ORDER[STEPS]);
            let last = m.next(STEPS);
            v = core::array::from_fn(|k| _mm_xor_si128(_mm_shuffle_epi32::<BACK>(v[k]), last[k]));
        }
        for k in 0..4 {
            // SAFETY: `cv` has 16 words, so 4 * k + 4 words are in bounds.
            unsafe { _mm_storeu_si128(cv[4 * k..].as_mut_ptr().cast(), v[k]) };
        }
    }};
}

#[target_feature(enable = "ssse3")]
fn blocks_ssse3(cv: &mut [u32; 16], blocks: &[u8]) {
    compress_blocks!(cv, blocks)
}

#[target_feature(enable = "avx2")]
fn blocks_avx2(cv: &mut [u32; 16], blocks: &[u8]) {
    compress_blocks!(cv, blocks)
}

fn ssse3(cv: &mut [u32; 16], blocks: &[u8]) {
    assert!(crate::x86::ssse3());
    // SAFETY: the CPU runs SSSE3.
    unsafe { blocks_ssse3(cv, blocks) }
}

fn avx2(cv: &mut [u32; 16], blocks: &[u8]) {
    assert!(crate::x86::avx2());
    // SAFETY: the CPU runs AVX2.
    unsafe { blocks_avx2(cv, blocks) }
}

#[inline]
pub(super) fn fastest() -> Option<Blocks> {
    if crate::x86::avx2() {
        Some(avx2)
    } else if crate::x86::ssse3() {
        Some(ssse3)
    } else {
        None
    }
}

impl<const N: usize> super::Lsh256<N> {
    #[cfg(feature = "kernels")]
    #[doc(hidden)]
    pub fn digest_ssse3(data: &[u8]) -> Option<[u8; N]> {
        crate::x86::ssse3().then(|| Self::digest_with(data, ssse3))
    }

    #[cfg(feature = "kernels")]
    #[doc(hidden)]
    pub fn digest_avx2(data: &[u8]) -> Option<[u8; N]> {
        crate::x86::avx2().then(|| Self::digest_with(data, avx2))
    }
}
