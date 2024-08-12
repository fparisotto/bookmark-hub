{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  buildInputs = [
    pkgs.rustc
    pkgs.cargo
    pkgs.rust-analyzer
    pkgs.openssl
    pkgs.pkg-config
    pkgs.hurl
    pkgs.trunk
    pkgs.tailwindcss
    pkgs.nodejs_22
  ];

  shellHook = ''
    export PKG_CONFIG_PATH=${pkgs.openssl.dev}/lib/pkgconfig
  '';
}
