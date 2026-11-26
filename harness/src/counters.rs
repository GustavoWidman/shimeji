#[derive(Clone, Copy)]
pub struct Counts {
    pub cycles: u64,
    pub instructions: u64,
    /// Nanoseconds the counters were enabled and actually counting. They
    /// differ when the kernel multiplexed the counters or the thread ran on
    /// a core whose PMU does not have them.
    enabled: u64,
    running: u64,
}

impl Counts {
    pub fn until(self, later: Counts) -> Option<Counts> {
        let enabled = later.enabled - self.enabled;
        let running = later.running - self.running;
        (enabled == running).then(|| Counts {
            cycles: later.cycles - self.cycles,
            instructions: later.instructions - self.instructions,
            enabled,
            running,
        })
    }
}

#[cfg(target_os = "linux")]
mod os {
    use super::Counts;

    const PERF_TYPE_HARDWARE: u32 = 0;
    const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
    const PERF_COUNT_HW_INSTRUCTIONS: u64 = 1;
    const PERF_FORMAT_TOTAL_TIME_ENABLED: u64 = 1;
    const PERF_FORMAT_TOTAL_TIME_RUNNING: u64 = 2;
    const PERF_FORMAT_GROUP: u64 = 8;
    const DISABLED: u64 = 1 << 0;
    const EXCLUDE_KERNEL: u64 = 1 << 5;
    const EXCLUDE_HV: u64 = 1 << 6;
    const PERF_EVENT_IOC_ENABLE: u64 = 0x2400;
    const PERF_IOC_FLAG_GROUP: u64 = 1;
    #[cfg(target_arch = "x86_64")]
    const SYS_PERF_EVENT_OPEN: i64 = 298;
    #[cfg(target_arch = "aarch64")]
    const SYS_PERF_EVENT_OPEN: i64 = 241;

    /// `struct perf_event_attr` up to `PERF_ATTR_SIZE_VER0`; the flag bits
    /// share one word.
    #[repr(C)]
    struct Attr {
        kind: u32,
        size: u32,
        config: u64,
        sample_period: u64,
        sample_type: u64,
        read_format: u64,
        flags: u64,
        wakeup_events: u32,
        bp_type: u32,
        config1: u64,
    }

    unsafe extern "C" {
        fn syscall(number: i64, ...) -> i64;
        fn ioctl(fd: i32, request: u64, ...) -> i32;
        fn read(fd: i32, buf: *mut std::ffi::c_void, count: usize) -> isize;
    }

    pub struct Counters(i32);

    fn open(config: u64, group: i32) -> Result<i32, String> {
        let attr = Attr {
            kind: PERF_TYPE_HARDWARE,
            size: size_of::<Attr>() as u32,
            config,
            sample_period: 0,
            sample_type: 0,
            read_format: PERF_FORMAT_GROUP
                | PERF_FORMAT_TOTAL_TIME_ENABLED
                | PERF_FORMAT_TOTAL_TIME_RUNNING,
            flags: EXCLUDE_KERNEL | EXCLUDE_HV | if group < 0 { DISABLED } else { 0 },
            wakeup_events: 0,
            bp_type: 0,
            config1: 0,
        };
        // SAFETY: `attr` is a valid PERF_ATTR_SIZE_VER0 attribute; pid 0 and
        // cpu -1 count the calling thread on any CPU.
        let fd = unsafe { syscall(SYS_PERF_EVENT_OPEN, &attr as *const Attr, 0, -1, group, 0) };
        match fd {
            fd if fd >= 0 => Ok(fd as i32),
            _ => Err(format!(
                "perf_event_open: {}",
                std::io::Error::last_os_error()
            )),
        }
    }

    impl Counters {
        pub fn open() -> Result<Counters, String> {
            let leader = open(PERF_COUNT_HW_CPU_CYCLES, -1)?;
            open(PERF_COUNT_HW_INSTRUCTIONS, leader)?;
            // SAFETY: plain ioctl on the group leader we own.
            if unsafe { ioctl(leader, PERF_EVENT_IOC_ENABLE, PERF_IOC_FLAG_GROUP) } != 0 {
                return Err(format!(
                    "enabling perf events: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Counters(leader))
        }

        pub fn read(&self) -> Counts {
            // nr, time_enabled, time_running, cycles, instructions.
            let mut buf = [0u64; 5];
            // SAFETY: writes at most `size_of_val(&buf)` bytes.
            let n = unsafe { read(self.0, buf.as_mut_ptr().cast(), size_of_val(&buf)) };
            assert_eq!(n, size_of_val(&buf) as isize, "reading perf events");
            Counts {
                cycles: buf[3],
                instructions: buf[4],
                enabled: buf[1],
                running: buf[2],
            }
        }
    }

    pub const METHOD: &str = "perf_event_open, PERF_TYPE_HARDWARE cycles and instructions \
        as one group on this thread, user mode only";
}

#[cfg(target_os = "macos")]
mod os {
    use super::Counts;

    const SYS_THREAD_SELFCOUNTS: i32 = 186;
    /// The fixed counters: instructions, then cycles.
    const FIXED: i32 = 1;

    unsafe extern "C" {
        fn syscall(number: i32, ...) -> i32;
    }

    pub struct Counters;

    impl Counters {
        pub fn open() -> Result<Counters, String> {
            Counters.try_read().map(|_| Counters)
        }

        fn try_read(&self) -> Result<Counts, String> {
            let mut buf = [0u64; 2];
            // SAFETY: writes at most `size_of_val(&buf)` bytes.
            let err = unsafe {
                syscall(
                    SYS_THREAD_SELFCOUNTS,
                    FIXED,
                    buf.as_mut_ptr(),
                    size_of_val(&buf),
                )
            };
            match err {
                0 => Ok(Counts {
                    cycles: buf[1],
                    instructions: buf[0],
                    enabled: 0,
                    running: 0,
                }),
                _ => Err(format!(
                    "thread_selfcounts: {}",
                    std::io::Error::last_os_error()
                )),
            }
        }

        pub fn read(&self) -> Counts {
            self.try_read().unwrap()
        }
    }

    pub const METHOD: &str = "thread_selfcounts, the fixed cycle and instruction counters \
        accumulated for this thread, user and kernel mode";
}

pub use os::{Counters, METHOD};
