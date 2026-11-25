//! The crate is `no_std`, so `std::is_x86_feature_detected!` is not available.

use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};
use core::sync::atomic::{AtomicU8, Ordering};

const KNOWN: u8 = 1;
const SSSE3: u8 = 2;
const AVX2: u8 = 4;

static FEATURES: AtomicU8 = AtomicU8::new(0);

#[inline]
pub(crate) fn ssse3() -> bool {
    cfg!(target_feature = "ssse3") || features() & SSSE3 != 0
}

#[inline]
pub(crate) fn avx2() -> bool {
    cfg!(target_feature = "avx2") || features() & AVX2 != 0
}

#[inline]
fn features() -> u8 {
    let known = FEATURES.load(Ordering::Relaxed);
    if known != 0 {
        return known;
    }
    let detected = detect();
    FEATURES.store(detected, Ordering::Relaxed);
    detected
}

#[cold]
fn detect() -> u8 {
    let max_leaf = __cpuid(0).eax;
    let leaf1 = __cpuid(1);
    let mut features = KNOWN;
    if leaf1.ecx & (1 << 9) != 0 {
        features |= SSSE3;
    }
    // AVX2 needs the CPU flag (leaf 7, EBX bit 5), AVX (leaf 1, ECX bit 28),
    // and an OS that enabled XSAVE (ECX bit 27) with XMM and YMM state in
    // XCR0 (bits 1 and 2).
    let xsave = leaf1.ecx & (1 << 27) != 0;
    let avx = leaf1.ecx & (1 << 28) != 0;
    let avx2 = max_leaf >= 7 && __cpuid_count(7, 0).ebx & (1 << 5) != 0;
    // SAFETY: `xgetbv` exists when CPUID reports OSXSAVE, checked first.
    if xsave && avx && avx2 && unsafe { xcr0() } & 0b110 == 0b110 {
        features |= AVX2;
    }
    features
}

#[target_feature(enable = "xsave")]
unsafe fn xcr0() -> u64 {
    // SAFETY: the caller checked OSXSAVE; register 0 always exists.
    unsafe { _xgetbv(0) }
}
