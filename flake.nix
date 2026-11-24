{
  description = "LSH-256 and LSH-512 (KS X 3262) in pure Rust";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devshell.url = "github:numtide/devshell";
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
        { pkgs, lib, ... }:
        {
          devshells.default.env = [
            {
              name = "TYPST_FONT_PATHS";
              value = "${pkgs.gyre-fonts}/share/fonts";
            }
          ]
          # nixpkgs builds Rust's standard library for macOS 14; rustc links
          # for 11.0 by default, and the linker warns on every std object.
          ++ lib.optional pkgs.stdenv.hostPlatform.isDarwin {
            name = "MACOSX_DEPLOYMENT_TARGET";
            value = "14.0";
          };
          devshells.default.packages = with pkgs; [
            cargo
            clippy
            rustc
            rustfmt
            clang
            # go-krypto's amd64 assembly reads its IVs with aligned SSE loads
            # from Go slices; Go 1.27 places those slices' arrays at 8-byte
            # alignment and the loads fault. Go 1.26 aligns them.
            go_1_26
            typst
          ];
        };
    };
}
