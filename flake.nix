{
  description = "snapper: Semantic line break formatter";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          pname = "snapper";
          version = "0.1.0";
          strictDeps = true;
        };

        cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "snapper-deps";
        });

        snapper = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });
      in {
        packages.default = snapper;

        checks.default = snapper;

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            rust-analyzer
            cargo-watch
          ];
        };
      });
}
