{
  description = "bookmark-hub";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
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
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "wasm32-unknown-unknown" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchainFor;

        # Add nightly rustfmt for formatting with unstable features
        nightlyRustfmt = pkgs.rust-bin.nightly.latest.rustfmt;

        # Separate crane instance using the nightly toolchain so that the
        # formatting check actually enforces unstable rustfmt features (e.g.
        # imports_granularity) declared in .rustfmt.toml. The stable craneLib
        # silently ignores those.
        nightlyCraneLib = (crane.mkLib pkgs).overrideToolchain
          (p: p.rust-bin.nightly.latest.default);

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
            pkgs.chromium
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
          cargoExtraArgs = "--package=cli";
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
            tag = "latest";
            created = "now";
            contents = [ serverPackage spaDistLayer pkgs.coreutils pkgs.bash pkgs.cacert pkgs.chromium pkgs.curl ];
            config = {
              Env = [
                "SPA_DIST=/data"
                "RUST_LOG=info"
              ];
              ExposedPorts = { "3000/tcp" = { }; };
              Cmd = [
                "${serverPackage}/bin/server"
              ];
              Labels = {
                "org.opencontainers.image.title" = "bookmark-hub";
                "org.opencontainers.image.description" = "Self-hosted bookmark manager with AI-powered tagging and search.";
                "org.opencontainers.image.version" = "0.1.0";
              };
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
              version = "0.2.126";
              hash = "sha256-H6Is3fiZVxZCfOMWK5dWMSrtn50VGv0sfdnsT+cTtyk=";
            };

            cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
              inherit src;
              inherit (src) pname version;
              hash = "sha256-VucqkXbCi4qtQzY/HrXiDnbSURsagPsdNVMn1Tw3UiY=";
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

          bookmark-hub-fmt = nightlyCraneLib.cargoFmt commonArgs;
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
            pkgs.cargo-audit
            nightlyRustfmt
          ];
        };
      });
}
