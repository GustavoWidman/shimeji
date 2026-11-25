//! Words cannot stay within their 128-bit lanes: the word pairs that three
//! consecutive steps mix connect all sixteen words, so at least one step in
//! any three must move words across lanes, and a schedule that never crosses
//! on two steps in a row is the least crossing possible.

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

const fn relabel(order: [usize; 4], map: &[usize; 4]) -> [usize; 4] {
    [
        position(map, order[0]),
        position(map, order[1]),
        position(map, order[2]),
        position(map, order[3]),
    ]
}

const CANONICAL: [usize; 4] = [0, 1, 2, 3];

const EVEN: [usize; 4] = [0, 2, 1, 3];

const ORDER: [[[usize; 4]; 2]; 2] = {
    assert!(matches!(quarter_map(2, 0), SIGMA_X) && matches!(quarter_map(3, 2), SIGMA_Y));
    assert!(within_lanes(reorder(relabel(EVEN, &SIGMA_Y), EVEN)));
    [[EVEN, EVEN], [relabel(EVEN, &SIGMA_X), EVEN]]
};

const fn reorder(from: [usize; 4], to: [usize; 4]) -> [usize; 4] {
    [
        position(&from, to[0]),
        position(&from, to[1]),
        position(&from, to[2]),
        position(&from, to[3]),
    ]
}

const fn within_lanes(from: [usize; 4]) -> bool {
    from[0] < 2 && from[1] < 2 && from[2] >= 2 && from[3] >= 2
}

const fn permute_imm(from: [usize; 4]) -> i32 {
    (from[0] | from[1] << 2 | from[2] << 4 | from[3] << 6) as i32
}

const fn transition(parity: usize) -> [[usize; 4]; 4] {
    let [x, y] = ORDER[parity];
    let [nx, ny] = ORDER[1 - parity];
    let mut moves = [[0; 4]; 4];
    let mut l = 0;
    while l < 4 {
        moves[0][l] = position(&y, SIGMA_X[nx[l]]);
        moves[1][l] = position(&y, SIGMA_Y[ny[l]]);
        moves[2][l] = position(&x, SIGMA_X[nx[l]]);
        moves[3][l] = position(&x, SIGMA_Y[ny[l]]);
        l += 1;
    }
    moves
}

const MOVES: [[[usize; 4]; 4]; 2] = {
    let even = transition(0);
    assert!(matches!(even[0], CANONICAL) && matches!(even[2], CANONICAL));
    assert!(within_lanes(even[1]) && within_lanes(even[3]));
    [even, transition(1)]
};

#[repr(align(32))]
struct Aligned<T>(T);

static SC_LANES: Aligned<[[u64; 8]; STEPS]> = Aligned({
    let mut sc = [[0; 8]; STEPS];
    let mut j = 0;
    while j < STEPS {
        let [x, y] = ORDER[j % 2];
        let mut l = 0;
        while l < 4 {
            sc[j][l] = SC[j][x[l]];
            sc[j][4 + l] = SC[j][4 + y[l]];
            l += 1;
        }
        j += 1;
    }
    sc
});

static GAMMA_SHUFFLE: Aligned<[[[u8; 32]; 2]; 2]> = Aligned({
    let mut index = [[[0; 32]; 2]; 2];
    let mut parity = 0;
    while parity < 2 {
        let ny = ORDER[1 - parity][1];
        let mut which = 0;
        while which < 2 {
            let gamma_base = if which == 0 { 4 } else { 0 };
            let mut l = 0;
            while l < 4 {
                let bytes = GAMMA[gamma_base + SIGMA_Y[ny[l]]] as usize / 8;
                let mut b = 0;
                while b < 8 {
                    index[parity][which][8 * l + b] = (8 * (l % 2) + (b + 8 - bytes) % 8) as u8;
                    b += 1;
                }
                l += 1;
            }
            which += 1;
        }
        parity += 1;
    }
    index
});

const TAU_LOW: i32 = tau_imm(0);
const TAU_HIGH: i32 = tau_imm(4);

const fn tau_imm(base: usize) -> i32 {
    let mut from = [0; 4];
    let mut l = 0;
    while l < 4 {
        from[l] = position(&EVEN, TAU[base + EVEN[l]] - base);
        l += 1;
    }
    permute_imm(from)
}

type SubMessage = [__m256i; 4];

#[inline]
#[target_feature(enable = "avx2")]
fn load_256(bytes: &[u8]) -> __m256i {
    let bytes: &[u8; 32] = bytes[..32].try_into().unwrap();
    // SAFETY: 32 readable bytes; `loadu` has no alignment requirement.
    unsafe { _mm256_loadu_si256(bytes.as_ptr().cast()) }
}

struct Message([SubMessage; 2]);

impl Message {
    #[inline]
    #[target_feature(enable = "avx2")]
    fn new(block: &[u8]) -> Self {
        const TO_EVEN: i32 = permute_imm(EVEN);
        let load = |at: usize| _mm256_permute4x64_epi64::<TO_EVEN>(load_256(&block[at..]));
        Self([
            core::array::from_fn(|k| load(32 * k)),
            core::array::from_fn(|k| load(128 + 32 * k)),
        ])
    }

    #[inline]
    #[target_feature(enable = "avx2")]
    fn next(&mut self, j: usize) -> SubMessage {
        let m = &mut self.0;
        if j > 1 {
            let (older, newer) = (m[j % 2], m[(j + 1) % 2]);
            m[j % 2] = [
                _mm256_add_epi64(newer[0], _mm256_permute4x64_epi64::<TAU_LOW>(older[0])),
                _mm256_add_epi64(newer[1], _mm256_permute4x64_epi64::<TAU_HIGH>(older[1])),
                _mm256_add_epi64(newer[2], _mm256_permute4x64_epi64::<TAU_LOW>(older[2])),
                _mm256_add_epi64(newer[3], _mm256_permute4x64_epi64::<TAU_HIGH>(older[3])),
            ];
        }
        let [p, q, r, s] = m[j % 2];
        if j.is_multiple_of(2) {
            return [p, q, r, s];
        }
        const X: i32 = permute_imm(reorder(EVEN, ORDER[1][0]));
        const Y: i32 = permute_imm(reorder(EVEN, ORDER[1][1]));
        [
            _mm256_permute4x64_epi64::<X>(p),
            _mm256_permute4x64_epi64::<Y>(q),
            _mm256_permute4x64_epi64::<X>(r),
            _mm256_permute4x64_epi64::<Y>(s),
        ]
    }
}

macro_rules! rotl {
    ($x:expr, $n:expr) => {{
        let x = $x;
        _mm256_or_si256(
            _mm256_slli_epi64::<{ $n as i32 }>(x),
            _mm256_srli_epi64::<{ 64 - $n as i32 }>(x),
        )
    }};
}

/// `x` unchanged, which the compiler takes to depend on `m`, so that it
/// computes `m` first. It emits no instruction.
///
/// [`step!`] passes `p`'s alpha rotation through this with the next step's
/// sub-message, so the compiler expands that sub-message (its `vpermq` and
/// adds) before the rotation, a step ahead of its use, instead of at
/// the end of the step, next to the use. The kernel then takes 1.4% fewer
/// cycles per block on an i9-13900K. The same tie on `q`'s rotation gains
/// nothing, so the gain is in the order the compiler then picks for the
/// instructions around the step boundaries, not in the lookahead itself.
#[inline]
#[target_feature(enable = "avx2")]
fn after(mut x: __m256i, m: &SubMessage) -> __m256i {
    // SAFETY: the assembly is empty; it reads and writes nothing but the
    // registers it names.
    unsafe {
        core::arch::asm!(
            "/* {0} {1} {2} {3} {4} */",
            inout(ymm_reg) x,
            in(ymm_reg) m[0],
            in(ymm_reg) m[1],
            in(ymm_reg) m[2],
            in(ymm_reg) m[3],
            options(pure, nomem, nostack, preserves_flags),
        );
    }
    x
}

macro_rules! step {
    ($v:expr, $m:expr, $j:expr, $next:expr) => {{
        let [p, q, r, s] = $v;
        let m: SubMessage = $m;
        // SAFETY: aligned 32-byte rows of statics.
        let (cp, cq, to_q, to_s) = unsafe {
            (
                _mm256_load_si256(SC_LANES.0[$j].as_ptr().cast()),
                _mm256_load_si256(SC_LANES.0[$j][4..].as_ptr().cast()),
                _mm256_load_si256(GAMMA_SHUFFLE.0[$j % 2][0].as_ptr().cast()),
                _mm256_load_si256(GAMMA_SHUFFLE.0[$j % 2][1].as_ptr().cast()),
            )
        };
        let (mut xp, mut xq) = (_mm256_xor_si256(p, m[0]), _mm256_xor_si256(q, m[1]));
        let (mut yr, mut ys) = (_mm256_xor_si256(r, m[2]), _mm256_xor_si256(s, m[3]));
        xp = _mm256_add_epi64(xp, yr);
        xq = _mm256_add_epi64(xq, ys);
        xp = _mm256_xor_si256(after(rotl!(xp, ALPHA[$j % 2]), &$next), cp);
        xq = _mm256_xor_si256(rotl!(xq, ALPHA[$j % 2]), cq);
        yr = rotl!(_mm256_add_epi64(yr, xp), BETA[$j % 2]);
        ys = rotl!(_mm256_add_epi64(ys, xq), BETA[$j % 2]);
        xp = _mm256_add_epi64(xp, yr);
        xq = _mm256_add_epi64(xq, ys);
        const MOVE: [[i32; 4]; 2] = [
            [
                permute_imm(MOVES[0][0]),
                permute_imm(MOVES[0][1]),
                permute_imm(MOVES[0][2]),
                permute_imm(MOVES[0][3]),
            ],
            [
                permute_imm(MOVES[1][0]),
                permute_imm(MOVES[1][1]),
                permute_imm(MOVES[1][2]),
                permute_imm(MOVES[1][3]),
            ],
        ];
        [
            _mm256_permute4x64_epi64::<{ MOVE[$j % 2][0] }>(xq),
            _mm256_shuffle_epi8(_mm256_permute4x64_epi64::<{ MOVE[$j % 2][1] }>(ys), to_q),
            _mm256_permute4x64_epi64::<{ MOVE[$j % 2][2] }>(xp),
            _mm256_shuffle_epi8(_mm256_permute4x64_epi64::<{ MOVE[$j % 2][3] }>(yr), to_s),
        ]
    }};
}

macro_rules! steps {
    ($v:ident, $m:ident, $next:ident; $($j:literal)+) => {
        let mut $next = $m.next(0);
        $(
            let current = $next;
            $next = $m.next($j + 1);
            $v = step!($v, current, $j, $next);
        )+
    };
}

#[target_feature(enable = "avx2")]
fn blocks_avx2(cv: &mut [u64; 16], blocks: &[u8]) {
    const TO_EVEN: i32 = permute_imm(EVEN);
    const FROM_EVEN: i32 = permute_imm(reorder(EVEN, CANONICAL));
    // SAFETY: `cv` has 16 words, so 4 * k + 4 words are in bounds.
    let mut v: [__m256i; 4] = core::array::from_fn(|k| {
        _mm256_permute4x64_epi64::<TO_EVEN>(unsafe {
            _mm256_loadu_si256(cv[4 * k..].as_ptr().cast())
        })
    });
    for block in blocks.as_chunks::<BLOCK>().0 {
        let mut m = Message::new(block);
        steps!(v, m, next; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27);
        v = core::array::from_fn(|k| _mm256_xor_si256(v[k], next[k]));
    }
    for k in 0..4 {
        let w = _mm256_permute4x64_epi64::<FROM_EVEN>(v[k]);
        // SAFETY: `cv` has 16 words, so 4 * k + 4 words are in bounds.
        unsafe { _mm256_storeu_si256(cv[4 * k..].as_mut_ptr().cast(), w) };
    }
}

mod narrow {
    use core::arch::x86_64::*;

    use super::{ALPHA, Aligned, BETA, BLOCK, GAMMA, SC, SIGMA_Y, STEPS, TAU};

    type Words = [__m128i; 8];

    static SC_ROWS: Aligned<[[u64; 8]; STEPS]> = Aligned(SC);

    const fn gamma_index(quarter: usize, half: usize) -> [u8; 16] {
        let gamma_base = if quarter == 1 { 4 } else { 0 };
        let mut index = [0u8; 16];
        let mut slot = 0;
        while slot < 2 {
            let old = SIGMA_Y[2 * half + slot];
            let bytes = GAMMA[gamma_base + old] as usize / 8;
            let from = 1 - slot;
            let mut b = 0;
            while b < 8 {
                index[8 * slot + b] = (8 * from + (b + 8 - bytes) % 8) as u8;
                b += 1;
            }
            slot += 1;
        }
        index
    }

    static GAMMA_INDEX: Aligned<[[u8; 16]; 4]> = Aligned([
        gamma_index(1, 0),
        gamma_index(1, 1),
        gamma_index(3, 0),
        gamma_index(3, 1),
    ]);

    const _: () = {
        assert!(matches!(super::SIGMA_X, [2, 0, 1, 3]) && matches!(SIGMA_Y, [0, 3, 2, 1]));
        assert!(matches!(TAU, [3, 2, 0, 1, 7, 4, 5, 6]));
    };

    #[inline(always)]
    fn load(bytes: &[u8]) -> __m128i {
        let bytes: &[u8; 16] = bytes[..16].try_into().unwrap();
        // SAFETY: 16 readable bytes; `loadu` is SSE2, part of x86-64.
        unsafe { _mm_loadu_si128(bytes.as_ptr().cast()) }
    }

    #[inline]
    #[target_feature(enable = "ssse3")]
    fn expand(older: &Words, newer: &Words) -> Words {
        let mut tau = [_mm_setzero_si128(); 8];
        for q in [0, 2] {
            tau[2 * q] = _mm_shuffle_epi32::<0x4e>(older[2 * q + 1]);
            tau[2 * q + 1] = older[2 * q];
        }
        for q in [1, 3] {
            tau[2 * q] = _mm_alignr_epi8::<8>(older[2 * q], older[2 * q + 1]);
            tau[2 * q + 1] = _mm_alignr_epi8::<8>(older[2 * q + 1], older[2 * q]);
        }
        core::array::from_fn(|k| _mm_add_epi64(newer[k], tau[k]))
    }

    macro_rules! rotl {
        ($x:expr, $n:expr) => {{
            let x = $x;
            _mm_or_si128(
                _mm_slli_epi64::<{ $n as i32 }>(x),
                _mm_srli_epi64::<{ 64 - $n as i32 }>(x),
            )
        }};
    }

    macro_rules! step {
        ($v:expr, $m:expr, $j:expr) => {{
            let mut v: Words = $v;
            let m: &Words = $m;
            for k in 0..8 {
                v[k] = _mm_xor_si128(v[k], m[k]);
            }
            let sc: *const __m128i = SC_ROWS.0[$j].as_ptr().cast();
            for k in 0..4 {
                v[k] = _mm_add_epi64(v[k], v[k + 4]);
            }
            for k in 0..4 {
                // SAFETY: four aligned 16-byte quarters of a 64-byte row.
                let c = unsafe { _mm_load_si128(sc.add(k)) };
                v[k] = _mm_xor_si128(rotl!(v[k], ALPHA[$j % 2]), c);
            }
            for k in 0..4 {
                v[k + 4] = _mm_add_epi64(v[k + 4], v[k]);
            }
            for k in 0..4 {
                v[k + 4] = rotl!(v[k + 4], BETA[$j % 2]);
            }
            for k in 0..4 {
                v[k] = _mm_add_epi64(v[k], v[k + 4]);
            }
            // SAFETY: aligned 16-byte rows of a static.
            let g = |k: usize| unsafe { _mm_load_si128(GAMMA_INDEX.0[k].as_ptr().cast()) };
            [
                _mm_unpacklo_epi64(v[3], v[2]),
                _mm_unpackhi_epi64(v[2], v[3]),
                _mm_shuffle_epi8(_mm_alignr_epi8::<8>(v[6], v[7]), g(0)),
                _mm_shuffle_epi8(_mm_alignr_epi8::<8>(v[7], v[6]), g(1)),
                _mm_unpacklo_epi64(v[1], v[0]),
                _mm_unpackhi_epi64(v[0], v[1]),
                _mm_shuffle_epi8(_mm_alignr_epi8::<8>(v[4], v[5]), g(2)),
                _mm_shuffle_epi8(_mm_alignr_epi8::<8>(v[5], v[4]), g(3)),
            ]
        }};
    }

    #[target_feature(enable = "ssse3")]
    pub(super) fn blocks(cv: &mut [u64; 16], blocks: &[u8]) {
        // SAFETY: `cv` has 16 words, so 2 * k + 2 words are in bounds.
        let mut v: Words =
            core::array::from_fn(|k| unsafe { _mm_loadu_si128(cv[2 * k..].as_ptr().cast()) });
        for block in blocks.as_chunks::<BLOCK>().0 {
            let mut m: [Words; 2] = [
                core::array::from_fn(|k| load(&block[16 * k..])),
                core::array::from_fn(|k| load(&block[128 + 16 * k..])),
            ];
            macro_rules! steps {
                ($($j:literal)+) => {$(
                    if $j > 1 {
                        m[$j % 2] = expand(&m[$j % 2], &m[($j + 1) % 2]);
                    }
                    v = step!(v, &m[$j % 2], $j);
                )+};
            }
            steps!(0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27);
            let last = expand(&m[0], &m[1]);
            v = core::array::from_fn(|k| _mm_xor_si128(v[k], last[k]));
        }
        for k in 0..8 {
            // SAFETY: `cv` has 16 words, so 2 * k + 2 words are in bounds.
            unsafe { _mm_storeu_si128(cv[2 * k..].as_mut_ptr().cast(), v[k]) };
        }
    }
}

fn ssse3(cv: &mut [u64; 16], blocks: &[u8]) {
    assert!(crate::x86::ssse3());
    // SAFETY: the CPU runs SSSE3.
    unsafe { narrow::blocks(cv, blocks) }
}

fn avx2(cv: &mut [u64; 16], blocks: &[u8]) {
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

impl<const N: usize> super::Lsh512<N> {
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
