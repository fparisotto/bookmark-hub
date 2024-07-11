{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.rustc
    pkgs.cargo
    pkgs.rust-analyzer
    pkgs.openssl
    pkgs.pkg-config
  ];

  shellHook = ''
    export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig
  '';
}
