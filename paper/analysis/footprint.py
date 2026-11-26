"""Measures the release build of the harness on this machine: the state and
peak stack that `harness footprint` reports, and the code bytes and
instruction mix of each compression function in the binary.

Run from `nix develop` at the repository root, on each benchmark machine:

    python3 paper/analysis/footprint.py

It builds the harness in release mode and writes
paper/data/footprint-<host>.json, which paper/analysis/analyze.py reads.

shimeji's compression functions are fully unrolled, so one call of a block
function's loop body is one block: its static instruction counts are per
block. KISA's compression function is a loop over step pairs that the C
compiler inlines into `update` and `final`; its code bytes are those two
functions together, and its instruction counts per block come from the
hardware counters instead.
"""

import json
import re
import socket
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target" / "release" / "harness"

# shimeji's block functions, matched in the Rust v0 mangled symbol name:
# the module path after the crate name, and the name of the code path the
# function runs for (the registry name in the result files).
SHIMEJI = re.compile(
    r"7shimeji6lsh(?P<family>256|512)(?P<path>8portable|13neon_schedule"
    r"|3x8611blocks_avx2|3x8612blocks_ssse3|3x866narrow6blocks)$"
)
PATHS = {
    "8portable": "shimeji-portable",
    "13neon_schedule": "shimeji",
    "3x8611blocks_avx2": "shimeji-avx2",
    "3x8612blocks_ssse3": "shimeji-ssse3",
    "3x866narrow6blocks": "shimeji-ssse3",
}
KISA = re.compile(r"^_?kisa_(?P<variant>[a-z0-9]+)_lsh(?P<family>256|512)_(?P<function>[a-z_]+)$")


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True, cwd=ROOT).stdout


def symbols():
    """Text symbols as {name: (address, size)}. ELF symbol tables record
    sizes; Mach-O ones do not, so a symbol there ends where the next
    one starts."""
    elf = BINARY.read_bytes()[:4] == b"\x7fELF"
    table = {}
    if elf:
        for line in run("nm", "-S", "--defined-only", str(BINARY)).splitlines():
            parts = line.split()
            if len(parts) == 4 and parts[2] in "tT":
                table[parts[3]] = (int(parts[0], 16), int(parts[1], 16))
        return table
    rows = []
    for line in run("nm", "-n", "--defined-only", str(BINARY)).splitlines():
        parts = line.split()
        if len(parts) == 3:
            rows.append((int(parts[0], 16), parts[1], parts[2]))
    for (address, kind, name), following in zip(rows, rows[1:]):
        if kind in "tT":
            table[name] = (address, following[0] - address)
    return table


def mnemonics(name, address, size):
    """Instruction counts of one function by mnemonic. On aarch64 a vector
    instruction is counted as `<mnemonic>.v` and a 128-bit load or store of
    q registers as `<mnemonic>.q`, apart from their scalar forms."""
    out = run(
        "objdump", "-d", "--no-show-raw-insn",
        f"--start-address={address:#x}", f"--stop-address={address + size:#x}",
        str(BINARY),
    )
    counts = Counter()
    for line in out.splitlines():
        match = re.match(r"^\s*[0-9a-f]+:\s+(\S+)\s*(.*)$", line)
        if not match:
            continue
        mnemonic, operands = match.groups()
        if re.search(r"\bv\d+\.\d*[bhsd]", operands):
            mnemonic += ".v"
        elif re.match(r"(ld|st)[pr]", mnemonic) and re.search(r"\bq\d+", operands):
            mnemonic += ".q"
        counts[mnemonic] += 1
    return counts


# Processes of `harness footprint` to run. Within one process every run of a
# path takes the same stack, but KISA's NEON LSH-256 takes 48 bytes more in
# some processes than in others, through the `memcpy` it calls; the largest
# stack of any process is kept.
PROCESSES = 8


def footprint():
    measured = {}
    for _ in range(PROCESSES):
        rows = run(str(BINARY), "footprint").splitlines()
        comment = [r for r in rows if r.startswith("#")]
        for kind, imp, family, n in (r.split("\t") for r in rows[2:]):
            key = (kind, imp, family)
            measured[key] = max(measured.get(key, 0), int(n))
    return comment, [
        {"kind": kind, "implementation": imp, "family": family, "bytes": n}
        for (kind, imp, family), n in measured.items()
    ]


def main():
    subprocess.run(["cargo", "build", "-p", "harness", "--release"], check=True, cwd=ROOT)
    comment, measured = footprint()
    table = symbols()
    functions = []
    for name, (address, size) in sorted(table.items(), key=lambda item: item[1]):
        if match := SHIMEJI.search(name):
            functions.append({
                "implementation": PATHS[match["path"]],
                "family": "lsh" + match["family"],
                "symbol": name,
                "bytes": size,
                "mnemonics": dict(mnemonics(name, address, size).most_common()),
            })
        elif match := KISA.match(name):
            functions.append({
                "implementation": "kisa-" + match["variant"],
                "family": "lsh" + match["family"],
                "function": match["function"],
                "symbol": name,
                "bytes": size,
            })
    host = socket.gethostname().split(".")[0]
    data = {
        "host": host,
        # The last commit that changed the code measured here.
        "commit": run("git", "log", "-1", "--format=%H", "--", "src", "harness", "Cargo.toml", "Cargo.lock").strip(),
        "rustc": run("rustc", "--version").strip(),
        "cc": run("clang", "--version").splitlines()[0],
        "footprint": comment,
        "measured": measured,
        "functions": functions,
    }
    out = ROOT / "paper" / "data" / f"footprint-{host}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(data, indent=1) + "\n")
    print(out, file=sys.stderr)


if __name__ == "__main__":
    main()
