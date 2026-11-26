//! macOS cannot pin a thread to a core, so the benchmark raises the thread to
//! the user-interactive QoS class, which keeps it on performance cores, and
//! records the core after every repetition so runs that migrated can be
//! filtered out.

use std::ffi::c_char;
use std::process::Command;

#[cfg(target_os = "macos")]
mod os {
    const QOS_CLASS_USER_INTERACTIVE: u32 = 0x21;

    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
        fn pthread_cpu_number_np(cpu: *mut usize) -> i32;
    }

    pub fn place(cpu: Option<usize>) -> Result<String, String> {
        if cpu.is_some() {
            return Err("macOS cannot pin a thread to a CPU; drop --cpu".into());
        }
        // SAFETY: plain FFI call on the current thread.
        match unsafe { pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0) } {
            0 => Ok("QoS user-interactive, unpinned".into()),
            err => Err(format!("pthread_set_qos_class_self_np failed: {err}")),
        }
    }

    pub fn current_cpu() -> usize {
        let mut cpu = 0;
        // SAFETY: writes one usize.
        unsafe { pthread_cpu_number_np(&mut cpu) };
        cpu
    }
}

#[cfg(target_os = "linux")]
mod os {
    unsafe extern "C" {
        fn sched_setaffinity(pid: i32, size: usize, mask: *const u64) -> i32;
        fn sched_getcpu() -> i32;
    }

    pub fn place(cpu: Option<usize>) -> Result<String, String> {
        let Some(cpu) = cpu else {
            return Ok("unpinned".into());
        };
        // A cpu_set_t: 1024 bits.
        let mut mask = [0u64; 16];
        *mask.get_mut(cpu / 64).ok_or("CPU index out of range")? |= 1 << (cpu % 64);
        // SAFETY: pid 0 is this thread; the mask is a full cpu_set_t.
        match unsafe { sched_setaffinity(0, size_of_val(&mask), mask.as_ptr()) } {
            0 => Ok(format!("pinned to CPU {cpu}")),
            _ => Err(format!(
                "sched_setaffinity to CPU {cpu} failed: {}",
                std::io::Error::last_os_error()
            )),
        }
    }

    pub fn current_cpu() -> usize {
        // SAFETY: no arguments.
        unsafe { sched_getcpu() as usize }
    }
}

pub use os::{current_cpu, place};

pub fn hostname() -> String {
    unsafe extern "C" {
        fn gethostname(name: *mut c_char, len: usize) -> i32;
    }
    let mut buf = [0u8; 256];
    // SAFETY: writes at most `buf.len()` bytes.
    unsafe { gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

pub fn cpu_model() -> String {
    if cfg!(target_os = "macos") {
        output("sysctl", &["-n", "machdep.cpu.brand_string"])
    } else {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|info| {
                info.lines().find_map(|line| {
                    line.strip_prefix("model name")
                        .map(|rest| rest.trim_start_matches([' ', '\t', ':']).to_owned())
                })
            })
            .unwrap_or_default()
    }
}

pub fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned()
        })
        .unwrap_or_default()
}
