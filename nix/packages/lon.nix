{
  stdenv,
  lib,
  rustPlatform,
  makeBinaryWrapper,
  pkg-config,
  openssl,
  nix,
  nix-prefetch-git,
  git,
  clippy,
  rustfmt,
  installShellFiles,
}:

let
  cargoToml = fromTOML (builtins.readFile ../../rust/lon/Cargo.toml);
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

  nativeBuildInputs = [
    makeBinaryWrapper
    pkg-config
    installShellFiles
  ];

  buildInputs = [
    openssl
  ];

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
  ''
  + lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
    installShellCompletion --cmd lon \
      --bash <(COMPLETE=bash $out/bin/lon) \
      --fish <(COMPLETE=fish $out/bin/lon) \
      --zsh <(COMPLETE=zsh $out/bin/lon)
  '';

  stripAllList = [ "bin" ];

  meta = with lib; {
    homepage = "https://github.com/nikstur/lon";
    license = licenses.mit;
    maintainers = with lib.maintainers; [ nikstur ];
    mainProgram = "lon";
  };
})
