{
  description = "Lock Nix Dependencies";

  inputs = {

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    systems.url = "github:nix-systems/default";

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    pre-commit-hooks-nix = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };

  };

  outputs =
    inputs@{
      self,
      flake-parts,
      systems,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } (
      { moduleWithSystem, ... }:
      {
        systems = import systems;

        imports = [ inputs.pre-commit-hooks-nix.flakeModule ];

        flake.nixosModules.lon-tests = moduleWithSystem (
          perSystem@{ config }:
          { ... }:
          {
            environment.systemPackages = [ perSystem.config.packages.lonTests ];
          }
        );

        perSystem =
          {
            config,
            system,
            pkgs,
            lib,
            ...
          }:
          {
            packages = import ./nix/packages { inherit pkgs; } // {
              default = config.packages.lon;
            };

            checks =
              {
                clippy = config.packages.default.overrideAttrs (
                  _: previousAttrs: {
                    pname = previousAttrs.pname + "-clippy";
                    nativeCheckInputs = (previousAttrs.nativeCheckInputs or [ ]) ++ [ pkgs.clippy ];
                    checkPhase = "cargo clippy";
                  }
                );
                rustfmt = config.packages.default.overrideAttrs (
                  _: previousAttrs: {
                    pname = previousAttrs.pname + "-rustfmt";
                    nativeCheckInputs = (previousAttrs.nativeCheckInputs or [ ]) ++ [ pkgs.rustfmt ];
                    checkPhase = "cargo fmt --check";
                  }
                );
              }
              // (import ./nix/tests {
                inherit pkgs;
                extraBaseModules = {
                  inherit (self.nixosModules) lon-tests;
                };
              });

            pre-commit = {
              check.enable = true;

              settings = {
                hooks = {
                  nixfmt = {
                    enable = true;
                    package = pkgs.nixfmt-rfc-style;
                  };
                };
              };
            };

            devShells.default = pkgs.mkShell {
              shellHook = ''
                ${config.pre-commit.installationScript}
              '';

              packages = [
                pkgs.niv
                pkgs.nixfmt-rfc-style
                pkgs.nix-prefetch-git
                pkgs.clippy
                pkgs.rustfmt
                pkgs.cargo-machete
                pkgs.cargo-edit
                pkgs.cargo-bloat
                pkgs.cargo-deny
                pkgs.cargo-cyclonedx
              ];

              inputsFrom = [ config.packages.default ];

              RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            };

          };
      }
    );
}
