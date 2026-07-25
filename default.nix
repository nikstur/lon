{
  system ? builtins.currentSystem,
}:
let
  sources = import ./lon.nix;
  pkgs = import sources.nixpkgs { inherit system; };
  lib = pkgs.lib;
in
rec {
  packages = lib.recurseIntoAttrs (import ./nix/packages { inherit pkgs; });

  checks = lib.recurseIntoAttrs {
    pre-commit = import ./nix/pre-commit.nix;

    inherit (packages.lon.tests) lint-format;

    tests = lib.recurseIntoAttrs (
      import ./nix/tests {
        inherit pkgs;
        extraBaseModules = {
          lon-tests = {
            environment.systemPackages = [ packages.lonTests ];
          };
        };
      }
    );
  };
}
