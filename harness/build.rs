//! Compiles KISA's LSH code from `refs/kisa` once per variant.
//!
//! Every variant defines the same four functions (`lsh256_init`, `_update`,
//! `_final`, `_digest`, or their `lsh512_` counterparts). Each variant is built
//! from a generated translation unit that renames them with `#define` to
//! `kisa_<variant>_<function>` and then includes the vendored `lsh-impl.c`
//! unchanged, so every variant links side by side.
//!
//! The flags below, and the full compiler command line `cc` derives from them,
//! are written to `$OUT_DIR/build-info.txt`, which the benchmark copies into
//! the header of every result file.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// Flags every KISA variant is compiled with. `cc`'s own defaults are turned
/// off, so these and the per-variant flags below are the whole command line.
const CFLAGS: &[&str] = &["-O3", "-fPIC"];

/// KISA variants: name, the target architecture it needs (none for portable
/// C), and its instruction-set flags.
/// The `xop` variant is left out: it needs AMD XOP hardware.
const VARIANTS: &[(&str, Option<&str>, &[&str])] = &[
    ("ref", None, &[]),
    ("neon", Some("aarch64"), &[]),
    ("sse2", Some("x86_64"), &["-msse2"]),
    ("ssse3", Some("x86_64"), &["-mssse3"]),
    ("avx2", Some("x86_64"), &["-mavx2"]),
];

const FAMILIES: &[&str] = &["lsh256", "lsh512"];
const FUNCTIONS: &[&str] = &["init", "update", "final", "digest"];
const COMPILER: &str = "clang";

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let neon_imm = manifest.join("c/neon_imm.h");

    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=c");
    println!(
        "cargo::rerun-if-changed={}",
        root.join("refs/kisa").display()
    );

    let mut info = String::new();
    writeln!(info, "cc: {}", first_line(COMPILER, &["--version"])).unwrap();
    writeln!(
        info,
        "rustc: {}",
        first_line(&env::var("RUSTC").unwrap(), &["-V"])
    )
    .unwrap();
    let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS")
        .unwrap_or_default()
        .replace('\x1f', " ");
    writeln!(info, "rustflags: {rustflags}").unwrap();
    writeln!(info, "rust profile: {}", release_profile(root)).unwrap();

    for &(variant, needs, flags) in VARIANTS {
        println!("cargo::rustc-check-cfg=cfg(kisa_{variant})");
        if needs.is_some_and(|needs| needs != arch) {
            continue;
        }
        println!("cargo::rustc-cfg=kisa_{variant}");
        for family in FAMILIES {
            let name = format!("kisa_{variant}_{family}");
            let dir = root.join("refs/kisa").join(family).join(variant);
            let mut unit = String::new();
            for function in FUNCTIONS {
                writeln!(
                    unit,
                    "#define {family}_{function} kisa_{variant}_{family}_{function}"
                )
                .unwrap();
            }
            if variant == "neon" {
                writeln!(unit, "#include \"{}\"", neon_imm.display()).unwrap();
            }
            writeln!(unit, "#include \"{}\"", dir.join("lsh-impl.c").display()).unwrap();
            let file = out.join(format!("{name}.c"));
            fs::write(&file, unit).unwrap();

            let mut build = cc::Build::new();
            build
                .compiler(COMPILER)
                .no_default_flags(true)
                .warnings(false)
                .file(&file);
            for flag in CFLAGS.iter().chain(flags) {
                build.flag(flag);
            }
            let args = command_line(&build);
            writeln!(info, "cflags kisa-{variant} {family}: {args}").unwrap();
            build.compile(&name);
        }
    }
    fs::write(out.join("build-info.txt"), info).unwrap();
}

fn first_line(program: &str, args: &[&str]) -> String {
    let output = Command::new(program).args(args).output().unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn command_line(build: &cc::Build) -> String {
    let tool = build.get_compiler();
    let args: Vec<_> = tool
        .args()
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.join(" ")
}

/// The Cargo profile this build uses, with the workspace's `[profile.release]`
/// settings when it is the release profile.
fn release_profile(root: &Path) -> String {
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    println!(
        "cargo::rerun-if-changed={}",
        root.join("Cargo.toml").display()
    );
    let settings: Vec<_> = manifest
        .lines()
        .skip_while(|line| line.trim() != "[profile.release]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let profile = env::var("PROFILE").unwrap();
    let opt = env::var("OPT_LEVEL").unwrap();
    match profile.as_str() {
        "release" => format!("release, opt-level = {opt}, {}", settings.join(", ")),
        _ => format!("{profile}, opt-level = {opt}"),
    }
}
