{ pkgs ? import <nixpkgs> {} }:

let

in pkgs.mkShell {
  name = "shotgun";

  buildInputs = let p = pkgs; in [
    p.cargo
    p.rust-analyzer
    p.rustc

    p.xorg.libX11
    p.xorg.libXrandr
    p.pkgconfig
  ];
}
