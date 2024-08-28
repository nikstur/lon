{ pkgs }:

rec {
  lon = pkgs.callPackage ./lon.nix { };
  lonTests = pkgs.callPackage ./lon-tests.nix { inherit lon; };
  # lonTests = lon.overrideAttrs (
  #   finalAttrs: previousAttrs: {
  #     nativeBuildInputs = previousAttrs.nativeBuildInputs or [ ] ++ [ pkgs.jq ];
  #     buildPhase = ''
  #       set -
  #       testBinary=$(${pkgs.rust.envVars.setEnv} cargo test --no-run -j $NIX_BUILD_CORES \
  #           --target ${pkgs.rust.envVars.rustHostPlatformSpec} \
  #           --offline \
  #           $cargoBuildProfileFlag \
  #           $cargoBuildNoDefaultFeaturesFlag \
  #           $cargoBuildFeaturesFlag \
  #           $cargoBuildFlags \
  #           --message-format=json | jq -r 'select(.target.kind[0] == "bin") | .executable')
  #     '';

  #     installPhase = ''
  #       install -D "$testBinary" $out/bin/lon-tests
  #     '';

  #     doCheck = false;
  #   }
  # );
}
