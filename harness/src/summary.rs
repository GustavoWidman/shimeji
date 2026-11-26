use std::collections::BTreeSet;
use std::fmt::Write as _;

use harness::Rng;

use crate::{options, parse_list};

const COLUMNS: [&str; 7] = [
    "implementation",
    "digest",
    "length",
    "ns_per_message",
    "cpu",
    "cycles_per_byte",
    "instructions_per_byte",
];

struct Row {
    implementation: String,
    digest: String,
    length: usize,
    ns_per_message: f64,
    cpu: usize,
    cycles_per_byte: Option<f64>,
    instructions_per_byte: Option<f64>,
}

struct Series {
    implementation: String,
    ns_per_message: Vec<f64>,
    cycles_per_byte: Vec<f64>,
    instructions_per_byte: Vec<f64>,
    cpus: BTreeSet<usize>,
}

struct Cell {
    digest: String,
    length: usize,
    series: Vec<Series>,
}

pub fn main(args: &[String]) -> Result<(), String> {
    let (opts, files) = options(&args[1..], &["baseline", "cpus", "resamples"])?;
    let [file] = files.as_slice() else {
        return Err("summary takes one results file".into());
    };
    let baseline = opts.get("baseline").map_or("kisa-ref", String::as_str);
    let cpus: Option<Vec<usize>> = opts
        .get("cpus")
        .map(|list| parse_list(list, "CPU"))
        .transpose()?;
    let resamples: usize = opts
        .get("resamples")
        .map_or(Ok(10_000), |n| n.parse().map_err(|_| "bad --resamples"))?;

    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut out = String::new();
    for line in text.lines().filter(|line| line.starts_with('#')) {
        writeln!(out, "{line}").unwrap();
    }
    let mut rows = parse(&text)?;
    if let Some(cpus) = &cpus {
        let before = rows.len();
        rows.retain(|row| cpus.contains(&row.cpu));
        writeln!(
            out,
            "# summary: kept {} of {before} repetitions, those that ended on CPUs {cpus:?}",
            rows.len()
        )
        .unwrap();
    }
    writeln!(
        out,
        "# summary: median and 95% bootstrap interval ({resamples} resamples); speedup = {baseline} median / median\n"
    )
    .unwrap();
    writeln!(
        out,
        "{:<11} {:>8} {:<12} {:>3} {:>9} {:>21} {:>12} {:>9} {:>9} {:>7} {:>21}  cpus",
        "digest",
        "length",
        "impl",
        "n",
        "ns/byte",
        "95% interval",
        "ns/message",
        "cyc/byte",
        "ins/byte",
        "speedup",
        "95% interval"
    )
    .unwrap();

    let mut rng = Rng::new(0x424f_4f54_5354_5250);
    for cell in cells(rows) {
        let base = cell.series.iter().find(|s| s.implementation == baseline);
        for series in &cell.series {
            let samples = &series.ns_per_message;
            let len = cell.length as f64;
            let med = median(samples.clone());
            let (lo, hi) = interval(
                (0..resamples)
                    .map(|_| median(resample(&mut rng, samples)))
                    .collect(),
            );
            let speedup = base.map(|base| {
                let point = median(base.ns_per_message.clone()) / med;
                let draws = (0..resamples)
                    .map(|_| {
                        median(resample(&mut rng, &base.ns_per_message))
                            / median(resample(&mut rng, samples))
                    })
                    .collect();
                (point, interval(draws))
            });
            let cpus: Vec<String> = series.cpus.iter().map(usize::to_string).collect();
            let speedup = match speedup {
                Some((point, (lo, hi))) => {
                    format!("{point:>7.3} {:>21}", format!("[{lo:.3}, {hi:.3}]"))
                }
                None => format!("{:>7} {:>21}", "-", "-"),
            };
            writeln!(
                out,
                "{:<11} {:>8} {:<12} {:>3} {:>9.4} {:>21} {:>12.1} {:>9} {:>9} {speedup}  {}",
                cell.digest,
                cell.length,
                series.implementation,
                samples.len(),
                med / len,
                format!("[{:.4}, {:.4}]", lo / len, hi / len),
                med,
                median_or_dash(&series.cycles_per_byte),
                median_or_dash(&series.instructions_per_byte),
                cpus.join(","),
            )
            .unwrap();
        }
    }
    print!("{out}");
    Ok(())
}

fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut lines = text
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty());
    let header: Vec<&str> = lines.next().ok_or("no header row")?.split('\t').collect();
    let column = |name: &str| header.iter().position(|&h| h == name);
    let [implementation, digest, length, ns_per_message, cpu] = COLUMNS[..5]
        .iter()
        .map(|&name| column(name).ok_or(format!("no column {name} in {header:?}")))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .unwrap();
    let (cycles, instructions) = (column(COLUMNS[5]), column(COLUMNS[6]));
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let bad = || format!("bad row: {line}");
            if fields.len() != header.len() {
                return Err(bad());
            }
            let counter = |i: Option<usize>| match i.map(|i| fields[i]) {
                None | Some("") => Ok(None),
                Some(field) => field.parse().map(Some).map_err(|_| bad()),
            };
            Ok(Row {
                implementation: fields[implementation].to_owned(),
                digest: fields[digest].to_owned(),
                length: fields[length].parse().map_err(|_| bad())?,
                ns_per_message: fields[ns_per_message].parse().map_err(|_| bad())?,
                cpu: fields[cpu].parse().map_err(|_| bad())?,
                cycles_per_byte: counter(cycles)?,
                instructions_per_byte: counter(instructions)?,
            })
        })
        .collect()
}

fn cells(rows: Vec<Row>) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    for row in rows {
        let cell = match cells
            .iter()
            .position(|c| c.digest == row.digest && c.length == row.length)
        {
            Some(i) => &mut cells[i],
            None => {
                cells.push(Cell {
                    digest: row.digest.clone(),
                    length: row.length,
                    series: Vec::new(),
                });
                cells.last_mut().unwrap()
            }
        };
        let series = match cell
            .series
            .iter()
            .position(|s| s.implementation == row.implementation)
        {
            Some(i) => &mut cell.series[i],
            None => {
                cell.series.push(Series {
                    implementation: row.implementation.clone(),
                    ns_per_message: Vec::new(),
                    cycles_per_byte: Vec::new(),
                    instructions_per_byte: Vec::new(),
                    cpus: BTreeSet::new(),
                });
                cell.series.last_mut().unwrap()
            }
        };
        series.ns_per_message.push(row.ns_per_message);
        series.cycles_per_byte.extend(row.cycles_per_byte);
        series
            .instructions_per_byte
            .extend(row.instructions_per_byte);
        series.cpus.insert(row.cpu);
    }
    cells
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    }
}

fn median_or_dash(xs: &[f64]) -> String {
    match xs {
        [] => "-".into(),
        xs => format!("{:.2}", median(xs.to_vec())),
    }
}

fn resample(rng: &mut Rng, xs: &[f64]) -> Vec<f64> {
    (0..xs.len()).map(|_| xs[rng.below(xs.len())]).collect()
}

fn interval(mut draws: Vec<f64>) -> (f64, f64) {
    draws.sort_by(f64::total_cmp);
    let at = |q: f64| draws[((draws.len() - 1) as f64 * q).round() as usize];
    (at(0.025), at(0.975))
}
