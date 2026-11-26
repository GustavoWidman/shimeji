{
  description = "LSH-256 and LSH-512 (KS X 3262) in pure Rust";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devshell.url = "github:numtide/devshell";
    # Rust nightly 2026-11-12 (rustc 1.102.0-nightly 3ef0c22cd), the rev the
    # NixOS configuration of the benchmark machines pins, so the dev shell's
    # rustc is the one installed on those systems.
    fenix = {
      url = "github:nix-community/fenix/0738efb391fea6fe32c886f5bac8e8ab90af3c81";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, devshell, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      imports = [ devshell.flakeModule ];

      perSystem =
        {
          pkgs,
          lib,
          inputs',
          ...
        }:
        let
          env = [
            {
              name = "TYPST_FONT_PATHS";
              value = "${pkgs.gyre-fonts}/share/fonts";
            }
            # Crypto++ source, which harness/build.rs builds with the dev
            # shell's clang. The commit is the one refs/fetch.sh takes the
            # LSH test vectors from.
            {
              name = "CRYPTOPP_SRC";
              value = pkgs.fetchFromGitHub {
                name = "cryptopp-5873a73a231c95771f5bbf3466a484952fb22780";
                owner = "weidai11";
                repo = "cryptopp";
                rev = "5873a73a231c95771f5bbf3466a484952fb22780";
                hash = "sha256-o6glEAtWHIL0HbzUhyBBGfor7ZJeoHYMZ9zMYSIElpI=";
              };
            }
            # digestpp, a header-only C++ hash library whose LSH
            # harness/c/digestpp.cpp exports to the harness, with the fix in
            # harness/c/digestpp-rotate.patch.
            {
              name = "DIGESTPP_SRC";
              value = pkgs.applyPatches {
                name = "digestpp-4beae7541f5c280389898ae6e6111028852f466a";
                src = pkgs.fetchFromGitHub {
                  owner = "kerukuro";
                  repo = "digestpp";
                  rev = "4beae7541f5c280389898ae6e6111028852f466a";
                  hash = "sha256-8MMj9Q16AtQmaBhRbqZOYaOv2R4Lk1xTmp2QIN9TpKQ=";
                };
                patches = [ ./harness/c/digestpp-rotate.patch ];
              };
            }
          ]
          # nixpkgs builds Rust's standard library for macOS 14; rustc links
          # for 11.0 by default, and the linker warns on every std object.
          ++ lib.optional pkgs.stdenv.hostPlatform.isDarwin {
            name = "MACOSX_DEPLOYMENT_TARGET";
            value = "14.0";
          };
          tools = with pkgs; [
            clang
            gnumake
            # go-krypto's amd64 assembly reads its IVs with aligned SSE loads
            # from Go slices; Go 1.27 places those slices' arrays at 8-byte
            # alignment and the loads fault. Go 1.26 aligns them.
            go_1_26
            typst
          ];
        in
        {
          devshells.default = {
            inherit env;
            packages = [
              (inputs'.fenix.packages.complete.withComponents [
                "cargo"
                "clippy"
                "rustc"
                "rustfmt"
                "rust-src"
              ])
            ]
            ++ tools;
          };
          # nixpkgs' stable toolchain. The crate uses no nightly features;
          # `nix develop .#stable -c cargo build -p shimeji --release` checks
          # that it still builds on stable Rust.
          devshells.stable = {
            inherit env;
            packages =
              (with pkgs; [
                cargo
                clippy
                rustc
                rustfmt
              ])
              ++ tools;
          };
        };
    };
}
