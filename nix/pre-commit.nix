let
  sources = import ../lon.nix;
  pkgs = import sources.nixpkgs { };
  pre-commit.run = pkgs.callPackage "${sources.pre-commit}/nix/run.nix" {
    inherit pkgs;
    tools = import "${sources.pre-commit}/nix/call-tools.nix" pkgs;
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
