//! The benchmark.
//!
//! After one warm-up period for the whole run, each (digest, message length)
//! cell first calibrates every selected implementation: the number of hashes
//! per timed loop is raised until one loop takes at least the minimum time,
//! so timer resolution is negligible.
//! Repetitions then alternate the implementation order (A B, B A, A B, ...) so
//! slow drifts in clock speed or temperature fall evenly on all of them.
//! `black_box` hides the message, the output buffer and the function pointer
//! from the optimizer, so every hash is an opaque call whose result is used.

use std::fs::{self, File};
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use harness::{BUILD_INFO, Digest, Implementation, Rng, implementations};

use crate::{options, parse_list, platform};

const LENGTHS: [usize; 7] = [16, 64, 256, 1024, 4096, 16384, 1 << 20];

/// Hashing before the first measurement, so the core is at its sustained
/// clock when the first cell is timed.
const WARMUP: Duration = Duration::from_millis(500);

pub fn main(args: &[String]) -> Result<(), String> {
    let (opts, rest) = options(
        &args[1..],
        &["impl", "digest", "len", "reps", "min-time-ms", "cpu", "out"],
    )?;
    if let Some(arg) = rest.first() {
        return Err(format!("unexpected argument {arg}"));
    }
    let names: Option<Vec<String>> = opts
        .get("impl")
        .map(|list| parse_list(list, "implementation"))
        .transpose()?;
    let digests: Vec<Digest> = match opts.get("digest") {
        Some(list) => list
            .split(',')
            .map(|name| Digest::parse(name.trim()).ok_or(format!("unknown digest {name}")))
            .collect::<Result<_, _>>()?,
        None => Digest::all().collect(),
    };
    let lengths: Vec<usize> = match opts.get("len") {
        Some(list) => parse_list(list, "length")?,
        None => LENGTHS.to_vec(),
    };
    let reps: usize = opts
        .get("reps")
        .map_or(Ok(20), |n| n.parse().map_err(|_| "bad --reps"))?;
    let min_ms: u64 = opts
        .get("min-time-ms")
        .map_or(Ok(20), |n| n.parse().map_err(|_| "bad --min-time-ms"))?;
    let cpu: Option<usize> = opts
        .get("cpu")
        .map(|n| n.parse().map_err(|_| "bad --cpu"))
        .transpose()?;

    let selected: Vec<&Implementation> = implementations()
        .filter(|imp| {
            names
                .as_ref()
                .is_none_or(|names| names.iter().any(|n| n == imp.name))
        })
        .collect();
    if let Some(names) = &names
        && let Some(missing) = names
            .iter()
            .find(|n| !selected.iter().any(|imp| imp.name == n.as_str()))
    {
        return Err(format!("no available implementation named {missing}"));
    }

    let placement = platform::place(cpu)?;
    let host = platform::hostname();
    let started = utc(SystemTime::now());
    let path = match opts.get("out") {
        Some(path) => PathBuf::from(path),
        None => {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("results");
            fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            dir.join(format!("{host}-{}.tsv", started.replace([':', '-'], "")))
        }
    };
    let file = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut out = BufWriter::new(file);

    let mut header = format!(
        "host: {host}\nos: {}\ncpu: {}\ncommit: {}\nstarted: {started}\ncommand: harness {}\nthread: {placement}\n\
         method: {reps} repetitions per (implementation, digest, length), each one loop of at least {min_ms} ms, \
         implementation order alternating ABBA, \
         after {} ms of warm-up\n",
        platform::output("uname", &["-srm"]),
        platform::cpu_model(),
        commit(),
        args.join(" "),
        WARMUP.as_millis(),
    );
    header.push_str(BUILD_INFO);
    for line in header.lines() {
        writeln!(out, "# {line}").map_err(io)?;
    }
    writeln!(
        out,
        "implementation\tdigest\tlength\trepetition\tns_per_byte\tns_per_message\tcpu\thost"
    )
    .map_err(io)?;

    let max = lengths.iter().copied().max().unwrap_or(0);
    let mut input = vec![0u8; max];
    Rng::new(0x4c53_482d_4245_4e43).fill(&mut input);
    let mut digest_out = [0u8; 64];
    let min_time = Duration::from_millis(min_ms);
    if let Some(imp) = selected.first() {
        let len = imp.digest_lengths[0];
        calibrate(
            imp,
            &input[..max.min(4096)],
            &mut digest_out[..len],
            WARMUP,
        );
    }

    for &digest in &digests {
        let imps: Vec<&Implementation> = selected
            .iter()
            .copied()
            .filter(|imp| imp.digests().any(|d| d == digest))
            .collect();
        for &len in &lengths {
            let message = &input[..len];
            let out_buf = &mut digest_out[..digest.len];
            let iters: Vec<u64> = imps
                .iter()
                .map(|imp| calibrate(imp, message, out_buf, min_time))
                .collect();
            eprintln!(
                "{} {len} bytes: {}",
                digest.name(),
                imps.iter()
                    .map(|imp| imp.name)
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            for rep in 0..reps {
                let order: Vec<usize> = match rep % 2 {
                    0 => (0..imps.len()).collect(),
                    _ => (0..imps.len()).rev().collect(),
                };
                for i in order {
                    let elapsed = time(imps[i], message, out_buf, iters[i]);
                    let core = platform::current_cpu();
                    let ns_per_message = elapsed.as_nanos() as f64 / iters[i] as f64;
                    writeln!(
                        out,
                        "{}\t{}\t{len}\t{rep}\t{:.6}\t{ns_per_message:.3}\t{core}\t{host}",
                        imps[i].name,
                        digest.name(),
                        ns_per_message / len as f64,
                    )
                    .map_err(io)?;
                }
            }
        }
    }
    out.flush().map_err(io)?;
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// Times `iters` hashes of `message` in one loop: the implementation's own
/// loop if it has one, otherwise a loop here calling `hash`.
#[inline(never)]
fn time(imp: &Implementation, message: &[u8], out: &mut [u8], iters: u64) -> Duration {
    let start = Instant::now();
    match imp.repeat {
        Some(repeat) => black_box(repeat)(black_box(message), black_box(&mut *out), iters),
        None => {
            for _ in 0..iters {
                black_box(imp.hash)(black_box(message), black_box(&mut *out));
            }
        }
    }
    start.elapsed()
}

/// The number of hashes for one loop to take at least `min`. The loops it
/// times also warm caches and branch predictors.
fn calibrate(imp: &Implementation, message: &[u8], out: &mut [u8], min: Duration) -> u64 {
    let mut iters = 1u64;
    loop {
        let elapsed = time(imp, message, out, iters);
        if elapsed >= min {
            return iters;
        }
        let scale = min.as_secs_f64() / elapsed.as_secs_f64().max(1e-9);
        iters = (iters as f64 * scale.min(100.0) * 1.05).ceil() as u64;
    }
}

/// The checked-out commit, with `-dirty` if tracked files differ from it.
fn commit() -> String {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    let hash = platform::output("git", &["-C", root, "rev-parse", "HEAD"]);
    let dirty = !platform::output(
        "git",
        &["-C", root, "status", "--porcelain", "--untracked-files=no"],
    )
    .is_empty();
    if dirty { format!("{hash}-dirty") } else { hash }
}

/// `time` as `YYYY-MM-DDTHH:MM:SSZ`.
fn utc(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn io(err: std::io::Error) -> String {
    err.to_string()
}
