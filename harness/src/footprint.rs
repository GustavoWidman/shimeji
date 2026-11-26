//! `harness footprint` prints the memory that shimeji's and KISA's code hold
//! and use: the bytes of the state a caller keeps between calls, and the peak
//! stack of hashing one message with each code path.
//!
//! Neither allocates, so these two are all the memory a hash takes. The peak
//! stack is measured, not read from the code: each path hashes a message on a
//! thread whose stack this program allocates and fills with a pattern, and the
//! deepest word that no longer holds the pattern marks the peak. The stack of
//! calling an empty function the same way is subtracted.

use std::ffi::c_void;

use shimeji::{Lsh256_256, Lsh512_512};

/// Bytes of the message each path hashes: several whole blocks of both
/// families and a padded last block.
const MESSAGE: usize = 1000;

const RUNS: usize = 32;

struct Path {
    name: &'static str,
    family: &'static str,
    hash: fn(&[u8]),
    available: fn() -> bool,
}

fn always() -> bool {
    true
}

macro_rules! shimeji_path {
    ($name:literal, $family:literal, $hasher:ty, $method:ident $(, $unwrap:ident)?; $available:expr) => {
        Path {
            name: $name,
            family: $family,
            hash: |m| {
                std::hint::black_box(<$hasher>::$method(m)$(.$unwrap())?);
            },
            available: $available,
        }
    };
}

/// A KISA variant's digest and context-size functions, renamed by `build.rs`.
macro_rules! kisa_path {
    ($name:literal, $family:literal, $digest:ident, $size:ident, $algtype:expr; $available:expr) => {{
        unsafe extern "C" {
            fn $digest(algtype: u32, data: *const u8, databitlen: usize, hashval: *mut u8) -> u32;
            fn $size() -> usize;
        }
        fn hash(message: &[u8]) {
            // KISA's SIMD variants need an aligned digest buffer; it is on
            // the heap so that it does not count as stack.
            #[repr(C, align(32))]
            struct Out([u8; 64]);
            let mut out = Box::new(Out([0; 64]));
            // SAFETY: KISA reads `8 * message.len()` bits of `message` and
            // writes at most 64 bytes to `out`.
            let err = unsafe {
                $digest(
                    $algtype,
                    message.as_ptr(),
                    8 * message.len(),
                    out.0.as_mut_ptr(),
                )
            };
            assert_eq!(err, 0, "KISA lsh_err {err:#x}");
            std::hint::black_box(&out);
        }
        // SAFETY: returns a constant.
        (
            Path {
                name: $name,
                family: $family,
                hash,
                available: $available,
            },
            unsafe { $size() },
        )
    }};
}

/// KISA's `lsh_type` for LSH-256-256 and LSH-512-512 (`lsh_def.h`).
const LSH256_256: u32 = 32;
const LSH512_512: u32 = 0x10000 | 64;

pub fn main(args: &[String]) -> Result<(), String> {
    if args.len() > 1 {
        return Err("footprint takes no arguments".into());
    }
    #[cfg(target_arch = "x86_64")]
    let (ssse3, avx2) = (
        || is_x86_feature_detected!("ssse3"),
        || is_x86_feature_detected!("avx2"),
    );
    let mut paths = vec![
        shimeji_path!("shimeji", "lsh256", Lsh256_256, digest; always),
        shimeji_path!("shimeji", "lsh512", Lsh512_512, digest; always),
        shimeji_path!("shimeji-portable", "lsh256", Lsh256_256, digest_portable; always),
        shimeji_path!("shimeji-portable", "lsh512", Lsh512_512, digest_portable; always),
        #[cfg(target_arch = "x86_64")]
        shimeji_path!("shimeji-ssse3", "lsh256", Lsh256_256, digest_ssse3, unwrap; ssse3),
        #[cfg(target_arch = "x86_64")]
        shimeji_path!("shimeji-ssse3", "lsh512", Lsh512_512, digest_ssse3, unwrap; ssse3),
        #[cfg(target_arch = "x86_64")]
        shimeji_path!("shimeji-avx2", "lsh256", Lsh256_256, digest_avx2, unwrap; avx2),
        #[cfg(target_arch = "x86_64")]
        shimeji_path!("shimeji-avx2", "lsh512", Lsh512_512, digest_avx2, unwrap; avx2),
    ];
    let mut states = vec![
        ("shimeji", "lsh256", size_of::<Lsh256_256>()),
        ("shimeji", "lsh512", size_of::<Lsh512_512>()),
    ];
    let kisa = [
        kisa_path!("kisa-ref", "lsh256", kisa_ref_lsh256_digest, kisa_ref_lsh256_context_size, LSH256_256; always),
        kisa_path!("kisa-ref", "lsh512", kisa_ref_lsh512_digest, kisa_ref_lsh512_context_size, LSH512_512; always),
        #[cfg(kisa_neon)]
        kisa_path!("kisa-neon", "lsh256", kisa_neon_lsh256_digest, kisa_neon_lsh256_context_size, LSH256_256; always),
        #[cfg(kisa_neon)]
        kisa_path!("kisa-neon", "lsh512", kisa_neon_lsh512_digest, kisa_neon_lsh512_context_size, LSH512_512; always),
        #[cfg(kisa_sse2)]
        kisa_path!("kisa-sse2", "lsh256", kisa_sse2_lsh256_digest, kisa_sse2_lsh256_context_size, LSH256_256; always),
        #[cfg(kisa_sse2)]
        kisa_path!("kisa-sse2", "lsh512", kisa_sse2_lsh512_digest, kisa_sse2_lsh512_context_size, LSH512_512; always),
        #[cfg(kisa_ssse3)]
        kisa_path!("kisa-ssse3", "lsh256", kisa_ssse3_lsh256_digest, kisa_ssse3_lsh256_context_size, LSH256_256; ssse3),
        #[cfg(kisa_ssse3)]
        kisa_path!("kisa-ssse3", "lsh512", kisa_ssse3_lsh512_digest, kisa_ssse3_lsh512_context_size, LSH512_512; ssse3),
        #[cfg(kisa_avx2)]
        kisa_path!("kisa-avx2", "lsh256", kisa_avx2_lsh256_digest, kisa_avx2_lsh256_context_size, LSH256_256; avx2),
        #[cfg(kisa_avx2)]
        kisa_path!("kisa-avx2", "lsh512", kisa_avx2_lsh512_digest, kisa_avx2_lsh512_context_size, LSH512_512; avx2),
    ];
    for (path, size) in kisa {
        states.push((path.name, path.family, size));
        paths.push(path);
    }

    // 64-byte aligned, so that every run takes the same path through code
    // that checks the alignment of its input.
    #[repr(align(64))]
    struct Message([u8; MESSAGE]);
    let mut message = Box::new(Message([0; MESSAGE]));
    harness::SplitMix64::new(1).fill(&mut message.0);
    let message = &message.0[..];
    let idle = stack_used(&|| {});
    println!(
        "# footprint: state is the bytes a caller holds between calls; stack is the peak stack of one LSH-256-256 or LSH-512-512 digest of a {MESSAGE}-byte message, the most of {RUNS} runs, beyond the {idle} bytes of calling an empty function the same way"
    );
    println!("kind\timplementation\tfamily\tbytes");
    for (name, family, bytes) in states {
        println!("state\t{name}\t{family}\t{bytes}");
    }
    for path in paths.iter().filter(|p| (p.available)()) {
        // A first call runs any one-time setup, such as shimeji's CPU feature
        // detection, outside the measurement.
        (path.hash)(message);
        // The most of several runs: a library function such as `memcpy`
        // may take a deeper path on some runs, on some cores.
        let used = (0..RUNS)
            .map(|_| stack_used(&|| (path.hash)(message)))
            .max()
            .unwrap();
        println!("stack\t{}\t{}\t{}", path.name, path.family, used - idle);
    }
    Ok(())
}

#[inline(always)]
fn stack_pointer() -> usize {
    let sp: usize;
    // SAFETY: reads a register.
    unsafe {
        #[cfg(target_arch = "aarch64")]
        std::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack, preserves_flags));
        #[cfg(target_arch = "x86_64")]
        std::arch::asm!("mov {}, rsp", out(reg) sp, options(nomem, nostack, preserves_flags));
    }
    sp
}

/// Bytes of stack below the caller's stack pointer that calling `f` uses,
/// measured on a thread whose stack this function allocates.
///
/// The C library's thread setup may already have used the stack below the
/// thread's first frame, so the thread paints every word below its stack
/// pointer just before calling `f`, and finds the deepest word that lost
/// the paint right after `f` returns.
fn stack_used(f: &(dyn Fn() + Sync)) -> usize {
    const SIZE: usize = 1 << 20;
    const PAINT: u64 = 0x5a17_c0de_5a17_c0de;

    struct Job<'a> {
        f: &'a (dyn Fn() + Sync),
        words: *mut u64,
        used: usize,
    }

    /// Paints every word from the lowest address of the stack up to this
    /// function's stack pointer. It keeps nothing below the stack pointer.
    #[inline(never)]
    extern "C" fn repaint(words: *mut u64) {
        let end = stack_pointer();
        for k in 0..(end - words as usize) / 8 {
            // SAFETY: below the stack pointer, within the stack allocation.
            unsafe { words.add(k).write_volatile(PAINT) };
        }
    }

    extern "C" fn run(job: *mut c_void) -> *mut c_void {
        // SAFETY: `stack_used` passes a `Job` that outlives the thread.
        let job: &mut Job = unsafe { &mut *job.cast() };
        let top = stack_pointer();
        repaint(job.words);
        (job.f)();
        // The stack grows down, from the end of the allocation.
        // SAFETY: every word below `top` was painted.
        let untouched = (0..SIZE / 8)
            .take_while(|&k| unsafe { job.words.add(k).read_volatile() } == PAINT)
            .count();
        job.used = top - (job.words as usize + 8 * untouched);
        std::ptr::null_mut()
    }

    let layout = std::alloc::Layout::from_size_align(SIZE, 1 << 16).unwrap();
    // SAFETY: the layout has a nonzero size.
    let stack = unsafe { std::alloc::alloc(layout) };
    assert!(!stack.is_null());
    let mut job = Job {
        f,
        words: stack.cast(),
        used: 0,
    };
    let mut attr = [0u64; 8];
    let mut thread: usize = 0;
    // SAFETY: `attr` is larger than `pthread_attr_t` on Linux and macOS and
    // is initialized before use; the stack is page-aligned, a multiple of
    // the page size, and outlives the thread, which is joined below.
    unsafe {
        assert_eq!(pthread_attr_init(attr.as_mut_ptr().cast()), 0);
        assert_eq!(
            pthread_attr_setstack(attr.as_mut_ptr().cast(), stack.cast(), SIZE),
            0
        );
        assert_eq!(
            pthread_create(
                &mut thread,
                attr.as_ptr().cast(),
                run,
                (&raw mut job).cast()
            ),
            0
        );
        assert_eq!(pthread_join(thread, std::ptr::null_mut()), 0);
        pthread_attr_destroy(attr.as_mut_ptr().cast());
        // SAFETY: allocated above with this layout; the thread has exited.
        std::alloc::dealloc(stack, layout);
    }
    job.used
}

unsafe extern "C" {
    fn pthread_attr_init(attr: *mut c_void) -> i32;
    fn pthread_attr_setstack(attr: *mut c_void, stack: *mut c_void, size: usize) -> i32;
    fn pthread_attr_destroy(attr: *mut c_void) -> i32;
    fn pthread_create(
        thread: *mut usize,
        attr: *const c_void,
        start: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> i32;
    fn pthread_join(thread: usize, result: *mut *mut c_void) -> i32;
}
