//! `harness bench` measures every registry implementation and writes one TSV
//! row per repetition; `harness summary` reduces such a file to medians and
//! confidence intervals.

mod bench;
mod counters;
mod platform;
mod summary;

use std::collections::HashMap;
use std::process::ExitCode;

const USAGE: &str = "\
usage:
  harness bench [--impl NAMES] [--digest NAMES] [--len BYTES] [--reps N]
                [--min-time-ms MS] [--cpu N] [--out FILE]

  Lists are comma-separated. Defaults: every available implementation, all
  six digests, lengths 16,64,256,1024,4096,16384,1048576, 20 repetitions,
  20 ms per timed loop, results/<host>-<UTC time>.tsv. --cpu pins the thread
  (Linux only).

  harness summary FILE [--baseline NAME] [--cpus LIST] [--resamples N]

  Prints the median and a 95% bootstrap interval per (digest, length,
  implementation), and the speedup over --baseline (default kisa-ref).
  --cpus keeps only repetitions that ended on the listed CPUs.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("bench") => bench::main(&args),
        Some("summary") => summary::main(&args),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Splits `--name value` pairs, rejecting names not in `known`; anything else
/// is returned as a positional argument.
fn options(
    args: &[String],
    known: &[&str],
) -> Result<(HashMap<String, String>, Vec<String>), String> {
    let mut named = HashMap::new();
    let mut positional = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.strip_prefix("--") {
            Some(name) if known.contains(&name) => {
                let value = args
                    .next()
                    .ok_or(format!("--{name} needs a value\n\n{USAGE}"))?;
                named.insert(name.to_owned(), value.clone());
            }
            Some(name) => return Err(format!("unknown option --{name}\n\n{USAGE}")),
            None => positional.push(arg.clone()),
        }
    }
    Ok((named, positional))
}

fn parse_list<T: std::str::FromStr>(list: &str, what: &str) -> Result<Vec<T>, String> {
    list.split(',')
        .map(|item| {
            item.trim()
                .parse()
                .map_err(|_| format!("bad {what}: {item}"))
        })
        .collect()
}
