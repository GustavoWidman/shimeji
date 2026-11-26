use std::fs::{self, File};
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use harness::{BUILD_INFO, Digest, Implementation, SplitMix64, implementations};

use crate::counters::{self, Counters};
use crate::{options, parse_list, platform};

const LENGTHS: [usize; 7] = [16, 64, 256, 1024, 4096, 16384, 1 << 20];

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
    let counters = Counters::open();
    let counted = match &counters {
        Ok(_) => counters::METHOD.to_owned(),
        Err(err) => format!("none ({err}); cycle and instruction columns are empty"),
    };
    let counters = counters.ok();
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
         after {} ms of warm-up\ncounters: {counted}\n",
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
        "implementation\tdigest\tlength\trepetition\tns_per_byte\tns_per_message\t\
         cycles_per_byte\tinstructions_per_byte\tcpu\thost"
    )
    .map_err(io)?;

    let max = lengths.iter().copied().max().unwrap_or(0);
    let mut input = vec![0u8; max];
    SplitMix64::new(0x4c53_482d_4245_4e43).fill(&mut input);
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
            let out_buf = &mut digest_out[..digest.bytes];
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
                    let before = counters.as_ref().map(Counters::read);
                    let elapsed = time(imps[i], message, out_buf, iters[i]);
                    let after = counters.as_ref().map(Counters::read);
                    let core = platform::current_cpu();
                    let ns_per_message = elapsed.as_nanos() as f64 / iters[i] as f64;
                    let bytes = (iters[i] * len as u64) as f64;
                    let counts = before.zip(after).and_then(|(b, a)| b.until(a));
                    let per_byte = |n: Option<u64>| {
                        n.map_or(String::new(), |n| format!("{:.4}", n as f64 / bytes))
                    };
                    let cycles = per_byte(counts.map(|c| c.cycles));
                    let instructions = per_byte(counts.map(|c| c.instructions));
                    writeln!(
                        out,
                        "{}\t{}\t{len}\t{rep}\t{:.6}\t{ns_per_message:.3}\t{cycles}\t{instructions}\t{core}\t{host}",
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

fn utc(time: SystemTime) -> String {
    let secs = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// The Gregorian date `days` after 1970-01-01, by Howard Hinnant's
/// `civil_from_days` algorithm.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn io(err: std::io::Error) -> String {
    err.to_string()
}
