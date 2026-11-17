//! The LSH construction, shared by LSH-256 and LSH-512.
//!
//! LSH is a wide-pipe Merkle-Damgård hash. A 16-word chaining variable absorbs
//! 32-word message blocks through `steps` rounds of a mix/permute step, with a
//! message schedule that expands the block's two 16-word halves into one
//! 16-word sub-message per step. The two families differ only in word size,
//! step count, rotation amounts and constants, so one macro defines both.

/// Defines an LSH family: its compression function and its streaming hasher.
///
/// `pairs` lists the step-pair indices after the first pair, so that the
/// compression function is fully unrolled: each step's word permutation then
/// compiles to register renaming instead of moves.
macro_rules! lsh {
    (
        $(#[$doc:meta])*
        hasher: $hasher:ident,
        word: $w:ty,
        steps: $steps:literal,
        pairs: [$($pair:literal)+],
        rotations: { even: ($alpha_e:literal, $beta_e:literal), odd: ($alpha_o:literal, $beta_o:literal) },
        gamma: $gamma:expr,
        sc0: $sc0:expr,
        ivs: { $($n:literal => $iv:expr),+ $(,)? },
    ) => {
        const WORD: usize = core::mem::size_of::<$w>();

        /// Bytes in one message block (32 words).
        pub const BLOCK: usize = 32 * WORD;

        const STEPS: usize = $steps;
        const GAMMA: [u32; 8] = $gamma;
        const TAU: [usize; 8] = [3, 2, 0, 1, 7, 4, 5, 6];
        const SIGMA: [usize; 16] = [6, 4, 5, 7, 12, 15, 14, 13, 2, 0, 1, 3, 8, 11, 10, 9];

        /// Step constants: each step's eight words are the previous step's
        /// words plus themselves rotated left by eight bits.
        const SC: [[$w; 8]; STEPS] = {
            let mut sc: [[$w; 8]; STEPS] = [[0; 8]; STEPS];
            sc[0] = $sc0;
            let mut j = 1;
            while j < STEPS {
                let mut l = 0;
                while l < 8 {
                    sc[j][l] = sc[j - 1][l].wrapping_add(sc[j - 1][l].rotate_left(8));
                    l += 1;
                }
                j += 1;
            }
            sc
        };

        /// One step: message addition, mix of each column pair, word permutation.
        #[inline(always)]
        fn step(mut s: [$w; 16], m: &[$w; 16], sc: &[$w; 8], alpha: u32, beta: u32) -> [$w; 16] {
            for k in 0..16 {
                s[k] ^= m[k];
            }
            for k in 0..8 {
                let mut x = s[k];
                let mut y = s[k + 8];
                x = x.wrapping_add(y).rotate_left(alpha) ^ sc[k];
                y = y.wrapping_add(x).rotate_left(beta);
                x = x.wrapping_add(y);
                y = y.rotate_left(GAMMA[k]);
                s[k] = x;
                s[k + 8] = y;
            }
            core::array::from_fn(|k| s[SIGMA[k]])
        }

        /// Next sub-message: `next[l] = newer[l] + older[tau(l)]`, per half.
        #[inline(always)]
        fn expand(older: [$w; 16], newer: &[$w; 16]) -> [$w; 16] {
            core::array::from_fn(|k| newer[k].wrapping_add(older[TAU[k % 8] + k / 8 * 8]))
        }

        #[inline(always)]
        fn load(bytes: &[u8]) -> [$w; 16] {
            core::array::from_fn(|k| {
                <$w>::from_le_bytes(bytes[WORD * k..WORD * (k + 1)].try_into().unwrap())
            })
        }

        /// Absorbs one message block into the chaining variable.
        #[inline(always)]
        pub(crate) fn compress(cv: &mut [$w; 16], block: &[u8; BLOCK]) {
            let mut even = load(&block[..BLOCK / 2]);
            let mut odd = load(&block[BLOCK / 2..]);
            let mut s = *cv;
            s = step(s, &even, &SC[0], $alpha_e, $beta_e);
            s = step(s, &odd, &SC[1], $alpha_o, $beta_o);
            $(
                even = expand(even, &odd);
                s = step(s, &even, &SC[2 * $pair], $alpha_e, $beta_e);
                odd = expand(odd, &even);
                s = step(s, &odd, &SC[2 * $pair + 1], $alpha_o, $beta_o);
            )+
            even = expand(even, &odd);
            for k in 0..16 {
                cv[k] = s[k] ^ even[k];
            }
        }

        $(#[$doc])*
        #[derive(Clone)]
        pub struct $hasher<const N: usize> {
            cv: [$w; 16],
            buf: [u8; BLOCK],
            len: usize,
        }

        impl<const N: usize> $hasher<N> {
            const IV: [$w; 16] = match N {
                $($n => $iv,)+
                _ => panic!("unsupported digest length"),
            };

            /// Starts a new hash.
            pub const fn new() -> Self {
                Self { cv: Self::IV, buf: [0; BLOCK], len: 0 }
            }

            /// Absorbs `data`.
            pub fn update(&mut self, mut data: &[u8]) {
                if self.len > 0 {
                    let take = (BLOCK - self.len).min(data.len());
                    self.buf[self.len..self.len + take].copy_from_slice(&data[..take]);
                    self.len += take;
                    data = &data[take..];
                    if self.len < BLOCK {
                        return;
                    }
                    compress(&mut self.cv, &self.buf);
                    self.len = 0;
                }
                let mut blocks = data.chunks_exact(BLOCK);
                for block in &mut blocks {
                    compress(&mut self.cv, block.try_into().unwrap());
                }
                let rest = blocks.remainder();
                self.buf[..rest.len()].copy_from_slice(rest);
                self.len = rest.len();
            }

            /// Pads the last block, absorbs it and returns the digest.
            ///
            /// LSH appends a single `0x80` byte and zeros, with no length field,
            /// and always ends with this padded block.
            pub fn finalize(mut self) -> [u8; N] {
                self.buf[self.len] = 0x80;
                self.buf[self.len + 1..].fill(0);
                compress(&mut self.cv, &self.buf);
                let mut h = [0u8; 8 * WORD];
                for k in 0..8 {
                    h[WORD * k..WORD * (k + 1)].copy_from_slice(&(self.cv[k] ^ self.cv[k + 8]).to_le_bytes());
                }
                h[..N].try_into().unwrap()
            }

            /// Hashes `data` in one call.
            pub fn digest(data: &[u8]) -> [u8; N] {
                let mut hasher = Self::new();
                hasher.update(data);
                hasher.finalize()
            }
        }

        impl<const N: usize> Default for $hasher<N> {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

pub(crate) use lsh;
