{
  description = "bookmark-hub";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib;

        rustToolchainFor = p: p.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchainFor;

        # Add nightly rustfmt for formatting with unstable features
        nightlyRustfmt = pkgs.rust-bin.nightly.latest.rustfmt;

        unfilteredRoot = ./.;

        src = lib.fileset.toSource {
          root = unfilteredRoot;
          fileset = lib.fileset.unions [
            (craneLib.fileset.commonCargoSources unfilteredRoot)
            (lib.fileset.fileFilter
              (file: lib.any file.hasExt [ "html" "scss" "sql" ])
              unfilteredRoot
            )
            (lib.fileset.maybeMissing ./assets)
          ];
        };

        commonArgs = {
          inherit src;

          pname = "bookmark-hub";
          version = "0.1.0";
          strictDeps = true;

          nativeBuildInputs = [
            pkgs.pkg-config
          ];

          buildInputs = [
            pkgs.openssl
          ];
        };

        nativeArgs = commonArgs // {
          pname = "bookmark-hub-native";
        };

        cargoArtifacts = craneLib.buildDepsOnly nativeArgs;

        serverPackage = craneLib.buildPackage (nativeArgs // {
          inherit cargoArtifacts;
          SPA_DIST = spaPackage;
        });

        cliPackage = craneLib.buildPackage (nativeArgs // {
          pname = "bookmark-hub-cli";
          inherit cargoArtifacts;
        });

        serverImage =
          let
            spaDistLayer = pkgs.runCommand "spa-dist-layer" { } ''
              mkdir -p $out/data
              cp ${spaPackage}/* $out/data/
            '';
          in
          pkgs.dockerTools.buildLayeredImage {
            name = "bookmark-hub";
            contents = [ serverPackage spaDistLayer pkgs.coreutils pkgs.bash pkgs.cacert ];
            config = {
              Env = [
                "SPA_DIST=/data"
              ];
              Cmd = [
                "${serverPackage}/bin/server"
              ];
            };
          };

        wasmArgs = commonArgs // {
          pname = "bookmark-hub-wasm";
          cargoExtraArgs = "--package=spa";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        };

        cargoArtifactsWasm = craneLib.buildDepsOnly (wasmArgs // {
          doCheck = false;
        });

        spaPackage = craneLib.buildTrunkPackage (wasmArgs // {
          pname = "bookmark-hub-client";
          cargoArtifacts = cargoArtifactsWasm;
          preBuild = ''
            cd ./spa
          '';
          postBuild = ''
            mv ./dist ..
            cd ..
          '';
          wasm-bindgen-cli = pkgs.buildWasmBindgenCli rec {
            src = pkgs.fetchCrate {
              pname = "wasm-bindgen-cli";
              version = "0.2.100";
              hash = "sha256-3RJzK7mkYFrs7C/WkhW9Rr4LdP5ofb2FdYGz1P7Uxog=";
              # hash = lib.fakeHash;
            };

            cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
              inherit src;
              inherit (src) pname version;
              hash = "sha256-qsO12332HSjWCVKtf1cUePWWb9IdYUmT+8OPj/XP2WE=";
              # hash = lib.fakeHash;
            };
          };
        });
      in
      {
        checks = {
          inherit serverPackage spaPackage;
          bookmark-hub-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            SPA_DIST = "";
          });

          bookmark-hub-fmt = craneLib.cargoFmt commonArgs;
        };

        packages = {
          default = serverPackage;
          containerImage = serverImage;
          bookmark-hub-cli = cliPackage;
        };

        apps.default = flake-utils.lib.mkApp {
          name = "server";
          drv = serverPackage;
        };

        apps.bookmark-hub-cli = flake-utils.lib.mkApp {
          name = "cli";
          drv = cliPackage;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          SPA_DIST = "spa/dist";

          # Override rustfmt to use nightly version
          shellHook = ''
            export PATH="${nightlyRustfmt}/bin:$PATH"
          '';

          packages = [
            pkgs.hurl
            pkgs.just
            pkgs.trunk
            pkgs.rust-analyzer
            nightlyRustfmt
          ];
        };
      });
}
