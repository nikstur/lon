{
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
      ${rust.envVars.setEnv} cargo test --test '*' --no-run -j $NIX_BUILD_CORES \
          --target ${rust.envVars.rustHostPlatformSpec} \
          --offline \
          $cargoBuildProfileFlag \
          $cargoBuildNoDefaultFeaturesFlag \
          $cargoBuildFeaturesFlag \
          $cargoBuildFlags
      )
    '';

    postInstall = ''
      find /build/source/target/${rust.envVars.rustHostPlatformSpec}/release/deps/integration-* \
        -type f \
        -executable \
        -execdir install -D {} $out/bin/lon-tests \;
    '';

    doCheck = false;

    meta.mainProgram = "lon-tests";
  }
)
