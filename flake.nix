{
  description = "bookmark-rs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        f = with fenix.packages.${system}; combine [
          stable.toolchain
          targets.wasm32-unknown-unknown.stable.rust-std
        ];
      in
      {
        devShells.default = with pkgs; mkShell {
          packages = with pkgs; [
            f
            hurl
            llvmPackages.bintools
            nodejs_22
            openssl
            pkg-config
            tailwindcss
            trunk
            wasm-pack
          ];
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
          BACKEND_URL = "http://localhost:3000";
        };
      }
    );
}
