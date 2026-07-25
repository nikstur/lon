{
  stdenv,
  rust,
  jq,
  lon,
}:

lon.overrideAttrs (
  finalAttrs: previousAttrs: {
    pname = "lon-tests";

    nativeBuildInputs = previousAttrs.nativeBuildInputs or [ ] ++ [ jq ];

    # Mostly taken from cargoBuildHook but only builds the test binary
    postBuild = ''
      (
      set -x
      ${rust.envVars.setEnv} cargo test --test '*' --no-run "''${flagsArray[@]}" -- --ignored
      )
    '';

    postInstall = previousAttrs.postInstall + ''
      find /build/source/target/${stdenv.targetPlatform.rust.rustcTarget}/release/deps/ \
        -name "integration-*" \
        -type f \
        -executable \
        -execdir install -D {} $out/bin/lon-tests \;
    '';

    doCheck = false;

    meta.mainProgram = "lon-tests";
  }
)
