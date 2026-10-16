let
  sources = import ../lon.nix;
  pre-commit = import (sources.pre-commit + "/nix/default.nix") {
    system = builtins.currentSystem;
    inherit (sources) nixpkgs;
    isFlakes = false;
  };
in
pre-commit.run {
  src = ../.;

  hooks = {
    nixfmt.enable = true;
    deadnix.enable = true;
  };
}
