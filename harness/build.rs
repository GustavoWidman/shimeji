//! Every variant defines the same four functions (`lsh256_init`, `_update`,
//! `_final`, `_digest`, or their `lsh512_` counterparts). Each variant is built
//! from a generated translation unit that renames them with `#define` to
//! `kisa_<variant>_<function>` and then includes the vendored `lsh-impl.c`
//! unchanged, so every variant links side by side.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

const CFLAGS: &[&str] = &["-O3", "-fPIC"];

const VARIANTS: &[(&str, Option<&str>, &[&str])] = &[
    ("ref", None, &[]),
    ("neon", Some("aarch64"), &[]),
    ("sse2", Some("x86_64"), &["-msse2"]),
    ("ssse3", Some("x86_64"), &["-mssse3"]),
    ("avx2", Some("x86_64"), &["-mavx2"]),
];

const FAMILIES: &[&str] = &["lsh256", "lsh512"];
const FUNCTIONS: &[&str] = &["init", "update", "final", "digest", "repeat"];
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
            writeln!(
                unit,
                "lsh_err {family}_repeat(const lsh_type algtype, const lsh_u8 *data, \
                 size_t databitlen, lsh_u8 *hashval, unsigned long long iters) {{\n\
                 \tlsh_err err = LSH_SUCCESS;\n\
                 \tfor (unsigned long long i = 0; i < iters && err == LSH_SUCCESS; i++)\n\
                 \t\terr = {family}_digest(algtype, data, databitlen, hashval);\n\
                 \treturn err;\n}}"
            )
            .unwrap();
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
    lsh_simd(root, &out, &arch, &mut info);
    pbkdf2_lsh(root, &out, &arch, &mut info);
    cryptopp(&manifest, &out, &mut info);
    digestpp(&manifest, &mut info);
    gokrypto(&manifest, &out, &mut info);
    fs::write(out.join("build-info.txt"), info).unwrap();
}

const LSH_SIMD: &[(&str, &str, &[&str])] = &[
    ("sse2", "SSE2", &["-msse2"]),
    ("ssse3", "SSSE3", &["-mssse3"]),
    ("avx2", "AVX2", &["-mavx2"]),
];

/// Compiles each `refs/lsh-simd` variant as KISA's code is compiled: every
/// variant defines the same `compress256`, `Init256`, `Update256`, `Final256`
/// and `Hash256` (or their `512` counterparts), so a generated unit renames
/// them to `lsh_simd_<variant>_lsh256_<function>`, includes the vendored file
/// unchanged, and defines `lsh_simd_<variant>_lsh256_repeat`, which calls
/// `Hash256` a given number of times.
fn lsh_simd(root: &Path, out: &Path, arch: &str, info: &mut String) {
    if arch != "x86_64" {
        return;
    }
    let refs = root.join("refs/lsh-simd");
    println!("cargo::rerun-if-changed={}", refs.display());
    for &(variant, upstream, flags) in LSH_SIMD {
        for bits in [256, 512] {
            let name = format!("lsh_simd_{variant}_lsh{bits}");
            let mut unit = String::new();
            for function in ["compress", "Init", "Update", "Final", "Hash"] {
                writeln!(unit, "#define {function}{bits} {name}_{function}").unwrap();
            }
            let source = refs
                .join(variant)
                .join(format!("lsh{bits}_ref_{upstream}_OPT.c"));
            writeln!(unit, "#include \"{}\"", source.display()).unwrap();
            writeln!(
                unit,
                "int {name}_repeat(int hashbitlen, const Byte *data, DataLength databitlen, \
                 Byte *hashval, unsigned long long iters) {{\n\
                 \tint err = SUCCESS;\n\
                 \tfor (unsigned long long i = 0; i < iters && err == SUCCESS; i++)\n\
                 \t\terr = Hash{bits}(hashbitlen, data, databitlen, hashval);\n\
                 \treturn err;\n}}"
            )
            .unwrap();
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
            writeln!(info, "cflags lsh-simd-{variant} lsh{bits}: {args}").unwrap();
            build.compile(&name);
        }
    }
}

const PBKDF2_LSH_URL: &str = "https://raw.githubusercontent.com/ondoli0312/PBKDF2-HMAC-LSH256/be7026c11701490b7219353d1289b480efccb9da";
const PBKDF2_LSH_FILES: &[(&str, &str)] = &[
    (
        "Type.h",
        "640503e3c948c958f17f37cce2032fdadd8d3025da8e240ac1f21332f9488a62",
    ),
    (
        "LSH256.h",
        "bfb515875a2709fcba9ad18628f890054095c73cb56aa37573aaaced429314a3",
    ),
    (
        "SIMD_LSH_256.c",
        "bddf60513f0b9e00cfb027d6c82f965360262f227f794b910bafc2e51cee5525",
    ),
];

/// `-fms-extensions` is for the code's `__declspec(align)` and `__int64`.
/// `LSH256.h` declares `SIMD_Compression` with one parameter and
/// `SIMD_LSH_256.c` defines it with two, which C rejects, so the generated
/// unit first includes the header with that declaration renamed; its
/// `#pragma once` then skips the `.c` file's include.
fn pbkdf2_lsh(root: &Path, out: &Path, arch: &str, info: &mut String) {
    if arch != "x86_64" {
        return;
    }
    let dir = root.join("refs/pbkdf2-lsh");
    println!("cargo::rerun-if-changed={}", dir.display());
    fs::create_dir_all(&dir).unwrap();
    for &(file, digest) in PBKDF2_LSH_FILES {
        let path = dir.join(file);
        if path.exists() && sha256(&path) == digest {
            continue;
        }
        let url = format!("{PBKDF2_LSH_URL}/{file}");
        let partial = dir.join(format!("{file}.part"));
        let fetched = Command::new("curl")
            .args(["-fsSL", "--retry", "3", "-o"])
            .arg(&partial)
            .arg(&url)
            .status()
            .is_ok_and(|status| status.success());
        assert!(
            fetched,
            "cannot download {url}. Choi and Seo's PBKDF2-HMAC-LSH256 code has no \
             license and is not in this repository; build.rs downloads it with \
             curl. Place {} from that commit in {} to build offline.",
            PBKDF2_LSH_FILES
                .iter()
                .map(|(file, _)| *file)
                .collect::<Vec<_>>()
                .join(", "),
            dir.display()
        );
        let got = sha256(&partial);
        assert_eq!(got, digest, "{url} does not have the pinned SHA-256");
        fs::rename(&partial, &path).unwrap();
    }

    let mut unit = String::new();
    writeln!(
        unit,
        "#define SIMD_Compression pbkdf2_lsh_unused_declaration\n\
         #include \"{header}\"\n\
         #undef SIMD_Compression\n\
         #include \"{source}\"\n\
         void pbkdf2_lsh_sse2_lsh256_repeat(const byte *msg, word msg_len, byte *output, \
         unsigned long long iters) {{\n\
         \tfor (unsigned long long i = 0; i < iters; i++)\n\
         \t\tSIMD_LSH256(msg, msg_len, output);\n}}",
        header = dir.join("LSH256.h").display(),
        source = dir.join("SIMD_LSH_256.c").display(),
    )
    .unwrap();
    let file = out.join("pbkdf2_lsh_sse2.c");
    fs::write(&file, unit).unwrap();

    let mut build = cc::Build::new();
    build
        .compiler(COMPILER)
        .no_default_flags(true)
        .warnings(false)
        .file(&file);
    for flag in CFLAGS.iter().chain(&["-msse2", "-fms-extensions"]) {
        build.flag(flag);
    }
    writeln!(info, "pbkdf2-lsh: {PBKDF2_LSH_URL}").unwrap();
    writeln!(
        info,
        "cflags pbkdf2-lsh-sse2 lsh256: {}",
        command_line(&build)
    )
    .unwrap();
    build.compile("pbkdf2_lsh_sse2");
}

fn sha256(path: &Path) -> String {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum is not on PATH");
    assert!(output.status.success(), "shasum {}", path.display());
    let line = String::from_utf8(output.stdout).unwrap();
    line.split_whitespace().next().unwrap().to_owned()
}

/// `-DNDEBUG` is what Crypto++'s GNUmakefile adds by default.
const CXXFLAGS: &[&str] = &["-O3", "-fPIC", "-DNDEBUG"];
const CXX: &str = "clang++";

/// The GNUmakefile adds `-mssse3` and `-mavx2` to the SSSE3 and AVX2 LSH
/// files on x86-64.
fn cryptopp(manifest: &Path, out: &Path, info: &mut String) {
    println!("cargo::rerun-if-env-changed=CRYPTOPP_SRC");
    let src = PathBuf::from(
        env::var("CRYPTOPP_SRC").expect("CRYPTOPP_SRC is not set; run cargo from `nix develop`"),
    );
    let dir = out.join(src.file_name().unwrap());
    if !dir.exists() {
        let copied = Command::new("cp").arg("-R").arg(&src).arg(&dir).status();
        assert!(copied.unwrap().success(), "copying {}", src.display());
        let writable = Command::new("chmod")
            .arg("-R")
            .arg("u+w")
            .arg(&dir)
            .status();
        assert!(writable.unwrap().success());
    }
    let cxxflags = format!("CXXFLAGS={}", CXXFLAGS.join(" "));
    let jobs = format!("-j{}", env::var("NUM_JOBS").unwrap_or("1".into()));
    let make = ["static", &format!("CXX={CXX}"), &cxxflags, "CPPFLAGS="];
    let output = Command::new("make")
        .arg(&jobs)
        .args(make)
        .current_dir(&dir)
        .output()
        .expect("make is not on PATH");
    assert!(
        output.status.success(),
        "building Crypto++ failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let commands = dir.join("lsh-commands.txt");
    let lsh: String = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(" -c lsh"))
        .map(|line| format!("{line}\n"))
        .collect();
    if !lsh.is_empty() {
        fs::write(&commands, lsh).unwrap();
    }

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .compiler(CXX)
        .no_default_flags(true)
        .warnings(false)
        .include(&dir)
        .file(manifest.join("c/cryptopp.cpp"));
    for flag in CXXFLAGS {
        build.flag(flag);
    }
    let shim = command_line(&build);
    build.compile("cryptopp_shim");
    println!("cargo::rustc-link-search=native={}", dir.display());
    println!("cargo::rustc-link-lib=static=cryptopp");

    writeln!(info, "cryptopp: {}", src.display()).unwrap();
    writeln!(info, "cryptopp build: make {}", make.join(" ")).unwrap();
    for line in fs::read_to_string(&commands).unwrap_or_default().lines() {
        writeln!(info, "cxxflags cryptopp: {line}").unwrap();
    }
    writeln!(info, "cxxflags cryptopp shim: {shim}").unwrap();
}

fn digestpp(manifest: &Path, info: &mut String) {
    println!("cargo::rerun-if-env-changed=DIGESTPP_SRC");
    let src = PathBuf::from(
        env::var("DIGESTPP_SRC").expect("DIGESTPP_SRC is not set; run cargo from `nix develop`"),
    );
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .compiler(CXX)
        .no_default_flags(true)
        .warnings(false)
        .include(&src)
        .file(manifest.join("c/digestpp.cpp"));
    for flag in CXXFLAGS {
        build.flag(flag);
    }
    writeln!(info, "digestpp: {}", src.display()).unwrap();
    writeln!(info, "digestpp patch: harness/c/digestpp-rotate.patch").unwrap();
    writeln!(info, "cxxflags digestpp shim: {}", command_line(&build)).unwrap();
    build.compile("digestpp_shim");
}

fn gokrypto(manifest: &Path, out: &Path, info: &mut String) {
    let dir = manifest.join("go");
    println!("cargo::rerun-if-changed={}", dir.display());
    let lib = out.join("libgokrypto.a");
    let args = [
        "build",
        "-buildmode=c-archive",
        "-trimpath",
        "-o",
        lib.to_str().unwrap(),
        ".",
    ];
    let env = [
        ("CC", COMPILER),
        ("CGO_ENABLED", "1"),
        ("GOFLAGS", "-mod=readonly"),
        ("GOTOOLCHAIN", "local"),
    ];
    let status = Command::new("go")
        .args(args)
        .envs(env)
        .current_dir(&dir)
        .status()
        .expect("go is not on PATH; run cargo from `nix develop`");
    assert!(status.success(), "go build failed");
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=gokrypto");

    let module = Command::new("go")
        .args(["list", "-m", "github.com/RyuaNerin/go-krypto"])
        .current_dir(&dir)
        .output()
        .unwrap();
    writeln!(info, "go: {}", first_line("go", &["version"])).unwrap();
    writeln!(
        info,
        "go-krypto: {}",
        String::from_utf8_lossy(&module.stdout).trim()
    )
    .unwrap();
    let env: Vec<_> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
    writeln!(
        info,
        "go build: {} go {}",
        env.join(" "),
        args[..3].join(" ")
    )
    .unwrap();
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
