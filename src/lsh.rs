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
        neon_message: $neon_message:ty,
    ) => {
        const WORD: usize = core::mem::size_of::<$w>();

        /// Bytes in one message block (32 words).
        pub const BLOCK: usize = 32 * WORD;

        const STEPS: usize = $steps;
        const GAMMA: [u32; 8] = $gamma;
        /// Rotation amounts of even and odd steps.
        const ALPHA: [u32; 2] = [$alpha_e, $alpha_o];
        const BETA: [u32; 2] = [$beta_e, $beta_o];
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

        /// The step constants for the portable code. On aarch64, LSH-512's
        /// go behind a pointer the optimizer cannot see through: with the
        /// values in sight, LLVM builds each 64-bit constant from a `mov`
        /// and three `movk`, while from memory one `ldp` loads two of them.
        /// LSH-256's 32-bit constants take two instructions each, and
        /// x86-64 loads any constant with one, so both stay immediates.
        #[inline(always)]
        fn sc() -> &'static [[$w; 8]; STEPS] {
            if cfg!(target_arch = "aarch64") && WORD == 8 {
                core::hint::black_box(&SC)
            } else {
                &SC
            }
        }

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

        /// A 16-word sub-message of the message schedule, held in whatever
        /// registers expand it most cheaply.
        pub(crate) trait SubMessage: Copy {
            /// The next sub-message: `next[l] = newer[l] + older[tau(l)]`, per
            /// half.
            fn expand(older: Self, newer: &Self) -> Self;
            /// The 16 words, as the step adds them to the chaining variable.
            fn words(&self) -> [$w; 16];
        }

        /// A sub-message in general-purpose registers.
        impl SubMessage for [$w; 16] {
            #[inline(always)]
            fn expand(older: Self, newer: &Self) -> Self {
                core::array::from_fn(|k| newer[k].wrapping_add(older[TAU[k % 8] + k / 8 * 8]))
            }
            #[inline(always)]
            fn words(&self) -> [$w; 16] {
                *self
            }
        }

        /// The 32 little-endian words of one block.
        #[cfg(any(feature = "kernels", not(all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))))]
        #[inline(always)]
        fn load(block: &[u8]) -> [$w; 32] {
            core::array::from_fn(|k| {
                <$w>::from_le_bytes(block[WORD * k..WORD * (k + 1)].try_into().unwrap())
            })
        }

        /// The two halves of a block's 32 words: the first two sub-messages.
        #[cfg(any(feature = "kernels", not(all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))))]
        #[inline(always)]
        fn halves(block: [$w; 32]) -> [[$w; 16]; 2] {
            [core::array::from_fn(|k| block[k]), core::array::from_fn(|k| block[16 + k])]
        }

        /// Absorbs one message block, given as its first two sub-messages,
        /// into the chaining variable, with the step constants `sc`.
        #[inline(always)]
        fn compress<M: SubMessage>(cv: &mut [$w; 16], [mut even, mut odd]: [M; 2], sc: &[[$w; 8]; STEPS]) {
            let mut s = *cv;
            s = step(s, &even.words(), &sc[0], ALPHA[0], BETA[0]);
            s = step(s, &odd.words(), &sc[1], ALPHA[1], BETA[1]);
            $(
                even = M::expand(even, &odd);
                s = step(s, &even.words(), &sc[2 * $pair], ALPHA[0], BETA[0]);
                odd = M::expand(odd, &even);
                s = step(s, &odd.words(), &sc[2 * $pair + 1], ALPHA[1], BETA[1]);
            )+
            let last = M::expand(even, &odd).words();
            for k in 0..16 {
                cv[k] = s[k] ^ last[k];
            }
        }

        /// A function that absorbs whole blocks into a chaining variable.
        type Blocks = fn(&mut [$w; 16], &[u8]);

        /// Absorbs whole blocks with the portable compression function.
        #[cfg(any(feature = "kernels", not(all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))))]
        fn portable(cv: &mut [$w; 16], blocks: &[u8]) {
            for block in blocks.chunks_exact(BLOCK) {
                compress(cv, halves(load(block)), sc());
            }
        }

        /// Absorbs whole blocks with the portable compression function, the
        /// message schedule expanded in NEON registers (see `neon.rs`).
        #[cfg(all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))]
        fn neon_schedule(cv: &mut [$w; 16], blocks: &[u8]) {
            for block in blocks.chunks_exact(BLOCK) {
                compress(cv, <$neon_message>::load(block), sc());
            }
        }

        /// The fastest block function the running CPU supports: the NEON
        /// message schedule on little-endian aarch64, SIMD code chosen at run
        /// time on x86-64, the portable code elsewhere.
        #[cfg(all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))]
        #[inline]
        fn fastest() -> Blocks {
            neon_schedule
        }

        #[cfg(target_arch = "x86_64")]
        #[inline]
        fn fastest() -> Blocks {
            x86::fastest().unwrap_or(portable)
        }

        #[cfg(not(any(target_arch = "x86_64", all(target_arch = "aarch64", target_feature = "neon", target_endian = "little"))))]
        #[inline]
        fn fastest() -> Blocks {
            portable
        }

        /// Pads the message's last, partial block `rest`, absorbs it with
        /// `blocks` and returns the digest.
        ///
        /// LSH appends a single `0x80` byte and zeros, with no length field,
        /// and always ends with this padded block.
        fn finish<const N: usize>(mut cv: [$w; 16], rest: &[u8], blocks: Blocks) -> [u8; N] {
            let mut last = [0u8; BLOCK];
            last[..rest.len()].copy_from_slice(rest);
            last[rest.len()] = 0x80;
            blocks(&mut cv, &last);
            let mut h = [0u8; 8 * WORD];
            for k in 0..8 {
                h[WORD * k..WORD * (k + 1)].copy_from_slice(&(cv[k] ^ cv[k + 8]).to_le_bytes());
            }
            h[..N].try_into().unwrap()
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
                    fastest()(&mut self.cv, &self.buf);
                    self.len = 0;
                }
                let whole = data.len() / BLOCK * BLOCK;
                if whole > 0 {
                    fastest()(&mut self.cv, &data[..whole]);
                }
                let rest = &data[whole..];
                self.buf[..rest.len()].copy_from_slice(rest);
                self.len = rest.len();
            }

            /// Pads the last block, absorbs it and returns the digest.
            pub fn finalize(self) -> [u8; N] {
                finish(self.cv, &self.buf[..self.len], fastest())
            }

            /// Hashes `data` in one call.
            pub fn digest(data: &[u8]) -> [u8; N] {
                Self::digest_with(data, fastest())
            }

            /// [`digest`](Self::digest) on the portable code, for testing and
            /// measuring it on CPUs that also run SIMD code.
            #[cfg(feature = "kernels")]
            #[doc(hidden)]
            pub fn digest_portable(data: &[u8]) -> [u8; N] {
                Self::digest_with(data, portable)
            }

            /// Hashes `data` with `blocks` absorbing whole blocks.
            fn digest_with(data: &[u8], blocks: Blocks) -> [u8; N] {
                let mut cv = Self::IV;
                let whole = data.len() / BLOCK * BLOCK;
                if whole > 0 {
                    blocks(&mut cv, &data[..whole]);
                }
                finish(cv, &data[whole..], blocks)
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
