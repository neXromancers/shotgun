{ pkgs ? import <nixpkgs> {} }:

let

in pkgs.mkShell {
  name = "shotgun";

  buildInputs = let p = pkgs; in [
    p.cargo
    p.rust-analyzer
    p.rustc
    p.rustfmt
    p.clippy

    p.cargo-release
  ];
}
