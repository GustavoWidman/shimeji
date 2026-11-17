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
        { pkgs, ... }:
        {
          devshells.default.env = [
            {
              name = "TYPST_FONT_PATHS";
              value = "${pkgs.gyre-fonts}/share/fonts";
            }
          ];
          devshells.default.packages = with pkgs; [
            cargo
            clippy
            rustc
            rustfmt
            clang
            typst
          ];
        };
    };
}
