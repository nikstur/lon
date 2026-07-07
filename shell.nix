let
  sources = import ./lon.nix;
  pkgs = import sources.nixpkgs { };
in
pkgs.mkShell {
  packages = [
    pkgs.niv
    pkgs.nixfmt
    pkgs.nix-prefetch-git
    pkgs.clippy
    pkgs.rustfmt
    pkgs.cargo-machete
    pkgs.cargo-edit
    pkgs.cargo-bloat
    pkgs.cargo-deny
    pkgs.cargo-cyclonedx
  ];

  inputsFrom = [
    (import ./default.nix { }).packages.lon
  ];

  shellHook = ''
    ${(import ./nix/pre-commit.nix).shellHook}
  '';

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
}
