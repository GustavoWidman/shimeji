/*
 * KISA's LSH-512 NEON code rotates through an inline function whose shift
 * count is a parameter:
 *
 *     vsriq_n_u64(vshlq_n_u64(value, rot), value, 64 - rot)
 *
 * GCC accepts this because every caller passes a constant and the immediate
 * is checked after inlining. Clang checks it before inlining and rejects it.
 * These replacements switch over every legal immediate; once inlined with a
 * constant count the switch folds to the one intrinsic, so clang emits the
 * same SHL and SRI instructions that GCC does.
 */
#include <arm_neon.h>

#define NEON_IMM_8(f, b) f(b) f(b + 1) f(b + 2) f(b + 3) f(b + 4) f(b + 5) f(b + 6) f(b + 7)
#define NEON_IMM_64(f)                                                                     \
	NEON_IMM_8(f, 0) NEON_IMM_8(f, 8) NEON_IMM_8(f, 16) NEON_IMM_8(f, 24)              \
	NEON_IMM_8(f, 32) NEON_IMM_8(f, 40) NEON_IMM_8(f, 48) NEON_IMM_8(f, 56)

static inline __attribute__((always_inline)) uint64x2_t neon_imm_vshlq_n_u64(uint64x2_t a, int n) {
	switch (n) {
#define NEON_IMM_CASE(k) case k: return vshlq_n_u64(a, k);
	NEON_IMM_64(NEON_IMM_CASE)
#undef NEON_IMM_CASE
	}
	__builtin_unreachable();
}

static inline __attribute__((always_inline)) uint64x2_t neon_imm_vsriq_n_u64(uint64x2_t a, uint64x2_t b, int n) {
	switch (n) {
#define NEON_IMM_CASE(k) case k + 1: return vsriq_n_u64(a, b, k + 1);
	NEON_IMM_64(NEON_IMM_CASE)
#undef NEON_IMM_CASE
	}
	__builtin_unreachable();
}

#undef vshlq_n_u64
#undef vsriq_n_u64
#define vshlq_n_u64 neon_imm_vshlq_n_u64
#define vsriq_n_u64 neon_imm_vsriq_n_u64
