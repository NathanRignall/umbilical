{
  lib,
  buildPackages,
  rustPlatform,
  makeWrapper,
}:

let
  # The page is plain HTML/JS for the guest's browser, so build it with the build machine's
  # node: identical output, and cross builds don't have to compile node for the target.
  inherit (buildPackages) buildNpmPackage importNpmLock;

  web = buildNpmPackage {
    pname = "umbilical-web";
    version = "0.1.0";
    src = ../web;
    # Hashes come straight from package-lock.json; nothing to update by hand.
    npmDeps = importNpmLock { npmRoot = ../web; };
    npmConfigHook = importNpmLock.npmConfigHook;
    installPhase = ''
      runHook preInstall
      cp -r dist $out
      runHook postInstall
    '';
  };
in
rustPlatform.buildRustPackage {
  pname = "umbilical";
  version = "0.1.0";
  src = ../server;
  cargoLock.lockFile = ../server/Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];

  # Bake the built page in; STATIC_DIR can still override it.
  postInstall = ''
    wrapProgram $out/bin/unifi-portal --set-default STATIC_DIR ${web}
  '';

  passthru = { inherit web; };

  meta = {
    description = "Custom UniFi external captive portal";
    mainProgram = "unifi-portal";
    platforms = lib.platforms.unix;
  };
}
