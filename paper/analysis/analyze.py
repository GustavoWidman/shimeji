"""Reduces the committed measurements to the numbers and figures the paper
prints. Run from `nix develop` at the repository root:

    python3 paper/analysis/analyze.py

Reads the benchmark runs in results/ (each machine's files in MACHINES, a
series taken from the first file that has it), results/asm-barrier/*.tsv
and paper/data/footprint-<host>.json (written by footprint.py on each
machine), and writes paper/data/numbers.json and paper/figures/*.svg.

Medians and 95% percentile-bootstrap intervals follow `harness summary`:
10,000 resamples of a series' time per message, and for a ratio of two
medians, each series resampled independently. The M6 keeps only the
repetitions that ran on its two Super cores, 6 and 7, as the runs'
summaries do.
"""

import csv
import json
import math
import os
from collections import defaultdict
from pathlib import Path

import matplotlib

matplotlib.use("svg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import font_manager  # noqa: E402
from matplotlib.ticker import FixedLocator, NullLocator  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "results"
DATA = ROOT / "paper" / "data"
FIGURES = ROOT / "paper" / "figures"

MACHINES = {
    "m6": {"files": ["single/altar.tsv"], "host": "altar", "cpus": {6, 7}},
    "i9": {"files": ["single/desktop-nixos.tsv"], "host": "desktop-nixos", "cpus": None},
}
# The paper reports one digest per family. The others differ only in IV
# and output length; `spread` below measures how little that changes.
DIGESTS = {"lsh256": "lsh256-256", "lsh512": "lsh512-512"}
FAMILY_DIGESTS = {
    "lsh256": ["lsh256-224", "lsh256-256"],
    "lsh512": ["lsh512-224", "lsh512-256", "lsh512-384", "lsh512-512"],
}
LENGTHS = [16, 64, 256, 1024, 4096, 16384, 1048576]
BLOCK = {"lsh256": 128, "lsh512": 256}
STEPS = {"lsh256": 26, "lsh512": 28}
RESAMPLES = 10_000
# A ratio closer to 1 than this prints with its interval.
CLOSE = 0.05

rng = np.random.default_rng(0x424F4F5453545250)


def read(path, cpus=None):
    """The header comments as {key: value} and the rows of one result file."""
    header, lines = {}, []
    for line in open(path):
        if line.startswith("# "):
            key, _, value = line[2:].partition(": ")
            header.setdefault(key, value.strip())
        elif line.strip():
            lines.append(line)
    rows = [
        r for r in csv.DictReader(lines, delimiter="\t")
        if cpus is None or int(r["cpu"]) in cpus
    ]
    return header, rows


def series(rows):
    """{(digest, length, implementation): {column: np.array}}."""
    out = defaultdict(lambda: defaultdict(list))
    for r in rows:
        s = out[(r["digest"], int(r["length"]), r["implementation"])]
        for col in ("ns_per_message", "ns_per_byte", "cycles_per_byte", "instructions_per_byte"):
            s[col].append(float(r[col]))
    return {k: {c: np.array(v) for c, v in s.items()} for k, s in out.items()}


def boot_medians(x):
    idx = rng.integers(0, len(x), size=(RESAMPLES, len(x)))
    return np.median(x[idx], axis=1)


def interval(draws):
    draws = np.sort(draws)
    at = lambda q: float(draws[int(round((len(draws) - 1) * q))])  # noqa: E731
    return at(0.025), at(0.975)


def ratio(base, other):
    """Median of `base` over median of `other`, with a 95% interval."""
    point = float(np.median(base) / np.median(other))
    lo, hi = interval(boot_medians(base) / boot_medians(other))
    return {"ratio": point, "lo": lo, "hi": hi}


def cell(s, length):
    ns = s["ns_per_message"]
    lo, hi = interval(boot_medians(ns))
    cyc = float(np.median(s["cycles_per_byte"]))
    ins = float(np.median(s["instructions_per_byte"]))
    return {
        "n": len(ns),
        "ns": float(np.median(ns)) / length,
        "ns_lo": lo / length,
        "ns_hi": hi / length,
        "cyc": cyc,
        "ins": ins,
        "ipc": ins / cyc,
    }


def is_other(name):
    return not name.startswith("shimeji")


def main():
    numbers = {"machines": {}, "cells": {}, "best": {}, "geomean": {}, "spread": {}}
    raw = {}
    for m, spec in MACHINES.items():
        header, rows = read(RESULTS / spec["files"][0], spec["cpus"])
        _, all_rows = read(RESULTS / spec["files"][0])
        s = series(rows)
        for file in spec["files"][1:]:
            for k, v in series(read(RESULTS / file, spec["cpus"])[1]).items():
                s.setdefault(k, v)
        raw[m] = s
        # The clock of each repetition is cycles over time. go-krypto's loop
        # runs inside the Go runtime, which takes the timed thread off the
        # CPU for part of the loop, so its cycles are left out.
        clocks = [
            r["cycles_per_byte"] / r["ns_per_byte"]
            for (d, l, i), r in s.items() if not i.startswith("go-krypto")
        ]
        go_clock = {
            str(length): float(np.median(np.concatenate([
                s[k]["cycles_per_byte"] / s[k]["ns_per_byte"]
                for k in s if k[1] == length and k[2].startswith("go-krypto")
            ])))
            for length in LENGTHS
        }
        numbers["machines"][m] = {
            "header": header,
            "repetitions": len(rows),
            "repetitions_all": len(all_rows),
            "reps": int(header["method"].split()[0]),
            "clock_ghz": float(np.median(np.concatenate(clocks))),
            "go_clock_ghz": go_clock,
            "implementations": sorted({k[2] for k in s}),
        }
        numbers["cells"][m] = {}
        numbers["best"][m] = {}
        numbers["geomean"][m] = {}
        numbers["spread"][m] = {}
        for fam, digest in DIGESTS.items():
            cells = {}
            best = {}
            for length in LENGTHS:
                here = {i: s[(digest, length, i)] for (d, l, i) in s if d == digest and l == length}
                cells[str(length)] = {i: cell(v, length) for i, v in here.items()}
                others = {i: v for i, v in here.items() if is_other(i)}
                name = min(others, key=lambda i: np.median(others[i]["ns_per_message"]))
                second = min(
                    (i for i in others if i != name),
                    key=lambda i: np.median(others[i]["ns_per_message"]),
                )
                best[str(length)] = {
                    "name": name,
                    "second": second,
                    **ratio(others[name]["ns_per_message"], here["shimeji"]["ns_per_message"]),
                    "close": False,
                    "vs_kisa": ratio(here["kisa-ref"]["ns_per_message"], here["shimeji"]["ns_per_message"]),
                }
                b = best[str(length)]
                b["close"] = abs(b["ratio"] - 1) < CLOSE
            numbers["cells"][m][fam] = cells
            numbers["best"][m][fam] = best
            numbers["geomean"][m][fam] = math.exp(
                sum(math.log(best[str(length)]["ratio"]) for length in LENGTHS) / len(LENGTHS)
            )
            # The other digests of the family differ only in IV and output
            # length: the geometric mean of shimeji's ratio to the fastest
            # other implementation, for each digest.
            spread = {}
            for d in FAMILY_DIGESTS[fam]:
                logs = []
                for length in LENGTHS:
                    here = {i: v for (dd, l, i), v in s.items() if dd == d and l == length}
                    t = {i: np.median(v["ns_per_message"]) for i, v in here.items()}
                    fastest = min(t[i] for i in t if is_other(i))
                    logs.append(math.log(fastest / t["shimeji"]))
                spread[d] = math.exp(sum(logs) / len(logs))
            numbers["spread"][m][fam] = spread

    numbers["barrier"] = barrier()
    # The known-answer vectors tests/kat.rs checks.
    kat = {
        f.name: sum(line.startswith("Digest:") for line in f.read_text().splitlines())
        for f in sorted((ROOT / "refs" / "kat").glob("*.txt"))
    }
    numbers["kat"] = {"files": kat, "total": sum(kat.values())}
    numbers["footprint"] = {
        m: json.loads((DATA / f"footprint-{spec['host']}.json").read_text())
        for m, spec in MACHINES.items()
    }
    numbers["derived"] = derived(numbers)
    DATA.mkdir(parents=True, exist_ok=True)
    (DATA / "numbers.json").write_text(json.dumps(numbers, indent=1, sort_keys=True) + "\n")
    figure(numbers)


def derived(numbers):
    """Per-block and per-step quantities the implementation section uses:
    instructions per block from the counters at 1 MiB, cycles per step, and
    instruction counts of the block functions from footprint.py."""
    out = {}
    for m in MACHINES:
        out[m] = {}
        for fam in DIGESTS:
            cells = numbers["cells"][m][fam]["1048576"]
            out[m][fam] = {
                i: {
                    "ins_block": c["ins"] * BLOCK[fam],
                    "cyc_block": c["cyc"] * BLOCK[fam],
                    "ins_step": c["ins"] * BLOCK[fam] / STEPS[fam],
                    "cyc_step": c["cyc"] * BLOCK[fam] / STEPS[fam],
                }
                for i, c in cells.items()
            }
    return out


def barrier():
    """Cycles per byte of the LSH-512 AVX2 kernel without the empty `asm!`
    over those with it, per length, with 95% intervals."""
    groups = defaultdict(lambda: defaultdict(list))
    for path in sorted((RESULTS / "asm-barrier").glob("*.tsv")):
        variant = path.name.split("-")[0]
        _, rows = read(path)
        for r in rows:
            groups[variant][int(r["length"])].append(
                (float(r["cycles_per_byte"]), float(r["instructions_per_byte"]))
            )
    out = {}
    for length in sorted(groups["with"]):
        w = np.array(groups["with"][length])
        o = np.array(groups["without"][length])
        out[str(length)] = {
            "n": len(w),
            **ratio(o[:, 0], w[:, 0]),
            "cyc_with": float(np.median(w[:, 0])),
            "cyc_without": float(np.median(o[:, 0])),
            "ins_with": float(np.median(w[:, 1])),
            "ins_without": float(np.median(o[:, 1])),
        }
    return out


# Names the paper uses for each registry entry.
LABELS = {
    "shimeji": "shimeji",
    "shimeji-portable": "shimeji, portable",
    "shimeji-ssse3": "shimeji, SSSE3",
    "shimeji-avx2": "shimeji, AVX2",
    "kisa-ref": "KISA C",
    "kisa-neon": "KISA NEON",
    "kisa-sse2": "KISA SSE2",
    "kisa-ssse3": "KISA SSSE3",
    "kisa-avx2": "KISA AVX2",
    "lsh-simd-sse2": "Kim et al. SSE2",
    "lsh-simd-ssse3": "Kim et al. SSSE3",
    "lsh-simd-avx2": "Kim et al. AVX2",
    "pbkdf2-lsh-sse2": "Choi–Seo SSE2",
    "cryptopp-ref": "Crypto++",
    "cryptopp-avx2": "Crypto++ AVX2",
    "digestpp": "digestpp",
    "lsh256-rs": "lsh256 crate",
    "allcrypt": "allcrypt",
    "go-krypto-neon": "go-krypto NEON",
    "go-krypto-ssse3": "go-krypto SSSE3",
    "go-krypto-avx2": "go-krypto AVX2",
}

# Grayscale styles: shimeji in black, the others in grays, with a marker per
# project (square KISA, triangle Kim et al., diamond go-krypto). Each
# implementation keeps its style in every panel.
DARK = dict(color="#555555", linestyle=(0, (4, 1.5)))
MID = dict(color="#7a7a7a", linestyle=(0, (1, 1)))
LIGHT = dict(color="#9e9e9e", linestyle=(0, (5, 1, 1, 1)))
STYLE = {
    "shimeji": dict(color="black", linestyle="-", marker="o", markerfacecolor="black", linewidth=1.3),
    "shimeji-portable": dict(color="black", linestyle="--", marker="o", markerfacecolor="white", linewidth=0.9),
    "kisa-ref": dict(**DARK, marker="s"),
    "kisa-avx2": dict(**DARK, marker="s"),
    "kisa-ssse3": dict(**MID, marker="s"),
    "kisa-neon": dict(**LIGHT, marker="s"),
    "go-krypto-neon": dict(**MID, marker="D"),
    "lsh-simd-ssse3": dict(**DARK, marker="^"),
    "lsh-simd-avx2": dict(**LIGHT, marker="^"),
}
MARKERS = {"kisa": "s", "lsh-simd": "^", "go-krypto": "D", "cryptopp": "v", "digestpp": "x", "lsh256": "+", "allcrypt": "*"}


def project(name):
    for p in MARKERS:
        if name.startswith(p):
            return p
    return name


def plotted(numbers, m, fam):
    """shimeji, its portable code, and the three fastest other
    implementations by geometric-mean time over the lengths."""
    cells = numbers["cells"][m][fam]
    names = [i for i in cells["16"] if is_other(i)]
    rank = sorted(names, key=lambda i: sum(math.log(cells[str(n)][i]["ns"]) for n in LENGTHS))
    return ["shimeji", "shimeji-portable"] + rank[:3]


def figure(numbers):
    fonts = Path(os.environ["TYPST_FONT_PATHS"]) / "opentype"
    for f in fonts.glob("texgyretermes-*.otf"):
        font_manager.fontManager.addfont(str(f))
    plt.rcParams.update({
        "font.family": "TeX Gyre Termes",
        "font.size": 7,
        "axes.linewidth": 0.5,
        "xtick.major.width": 0.5,
        "ytick.major.width": 0.5,
        "xtick.minor.width": 0.4,
        "ytick.minor.width": 0.4,
        "lines.markersize": 3,
        "svg.fonttype": "none",
        "svg.hashsalt": "shimeji",
        "legend.fontsize": 6.5,
        "legend.frameon": False,
    })
    FIGURES.mkdir(parents=True, exist_ok=True)
    width = 3.5
    fig, axes = plt.subplots(2, 2, figsize=(width, 4.1), sharex=True)
    panels = [("m6", "lsh256"), ("m6", "lsh512"), ("i9", "lsh256"), ("i9", "lsh512")]
    clock = numbers["machines"]["i9"]["clock_ghz"]
    used = {}
    for ax, (m, fam), letter in zip(axes.flat, panels, "abcd"):
        cells = numbers["cells"][m][fam]
        names = plotted(numbers, m, fam)
        used[f"{m}-{fam}"] = names
        for name in names:
            # On the i9 every core ran at its fixed base clock, so cycles are
            # time times that clock, also for go-krypto, whose thread is off
            # the CPU for part of its loop. The M6 has no fixed clock and
            # counts cycles per thread.
            y = [
                cells[str(n)][name]["ns"] * clock if m == "i9" else cells[str(n)][name]["cyc"]
                for n in LENGTHS
            ]
            style = STYLE.get(name, dict(**LIGHT, marker=MARKERS.get(project(name), "o")))
            style = {"markerfacecolor": "white", "linewidth": 0.8, **style}
            ax.plot(LENGTHS, y, label=LABELS[name], markeredgewidth=0.6, **style)
        ax.set_xscale("log", base=2)
        ax.set_yscale("log")
        ax.xaxis.set_major_locator(FixedLocator(LENGTHS))
        ax.xaxis.set_minor_locator(NullLocator())
        ax.set_xticklabels(["16", "64", "256", "1K", "4K", "16K", "1M"])
        ticks = [1, 2, 3, 5, 10, 20, 50, 100, 200]
        lo = min(min(line.get_ydata()) for line in ax.get_lines())
        hi = max(max(line.get_ydata()) for line in ax.get_lines())
        # Room above the curves for the legend in the upper right.
        top = hi * 3.2
        ax.set_ylim(lo / 1.15, top)
        ax.yaxis.set_major_locator(FixedLocator([t for t in ticks if lo / 1.15 <= t <= top]))
        ax.yaxis.set_minor_locator(NullLocator())
        ax.yaxis.set_major_formatter(matplotlib.ticker.FormatStrFormatter("%g"))
        ax.tick_params(length=2, pad=1.5)
        ax.grid(True, which="major", color="#d0d0d0", linewidth=0.4)
        ax.set_axisbelow(True)
        for side in ("top", "right"):
            ax.spines[side].set_visible(False)
        ax.text(0.0, 1.03, f"({letter})", transform=ax.transAxes, ha="left", va="bottom")
        ax.legend(loc="upper right", handlelength=2.6, borderaxespad=0.1, labelspacing=0.2, handletextpad=0.4)
    for ax in axes[1]:
        ax.set_xlabel("Message length (bytes)")
    for ax in axes[:, 0]:
        ax.set_ylabel("Cycles per byte")
    fig.tight_layout(pad=0.2, h_pad=0.6, w_pad=0.6)
    fig.savefig(FIGURES / "cycles-per-byte.svg", metadata={"Date": None})
    plt.close(fig)
    numbers["plotted"] = used
    (DATA / "numbers.json").write_text(json.dumps(numbers, indent=1, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
