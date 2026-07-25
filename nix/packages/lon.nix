{
  lib,
  rustPlatform,
  makeBinaryWrapper,
  nix,
  nix-prefetch-git,
  git,
  clippy,
  rustfmt,
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../../rust/lon/Cargo.toml);
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  src = lib.sourceFilesBySuffices ../../rust/lon [
    ".rs"
    ".toml"
    ".lock"
    ".nix"
    ".json" # Test fixtures
  ];

  cargoLock = {
    lockFile = ../../rust/lon/Cargo.lock;
  };

  nativeBuildInputs = [ makeBinaryWrapper ];

  checkFlags = [ "--show-output" ];
  nativeCheckInputs = [
    git
    nix
    nix-prefetch-git
  ];

  passthru.tests = {
    lint-format = finalAttrs.finalPackage.overrideAttrs (
      _: previousAttrs: {
        pname = previousAttrs.pname + "-lint-format";
        nativeCheckInputs = (previousAttrs.nativeCheckInputs or [ ]) ++ [
          clippy
          rustfmt
        ];
        checkPhase = ''
          cargo clippy
          cargo fmt --check
        '';
      }
    );
  };

  postInstall = ''
    wrapProgram $out/bin/lon --prefix PATH : ${
      lib.makeBinPath [
        nix
        nix-prefetch-git
        git
      ]
    }
  '';

  stripAllList = [ "bin" ];

  meta = with lib; {
    homepage = "https://github.com/nikstur/lon";
    license = licenses.mit;
    maintainers = with lib.maintainers; [ nikstur ];
    mainProgram = "lon";
  };
})
