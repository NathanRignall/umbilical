{
  description = "umbilical: custom UniFi external captive portal";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-linux"
        "x86_64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAllSystems (pkgs: rec {
        umbilical = pkgs.callPackage ./nix/package.nix { };
        default = umbilical;
      });

      nixosModules.default = import ./nix/module.nix;

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            nodejs_22
          ];
        };
      });

      checks = nixpkgs.lib.genAttrs [ "x86_64-linux" "aarch64-linux" ] (system: {
        vm = nixpkgs.legacyPackages.${system}.testers.runNixOSTest (import ./nix/test.nix self);
      });

      formatter = forAllSystems (pkgs: pkgs.nixfmt);
    };
}
