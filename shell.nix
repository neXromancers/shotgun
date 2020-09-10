{ ... } @ args: let
  inherit (import ./nix args) nixpkgs pkgs;
in nixpkgs.mkShell {
  name = "nexromancers-shotgun-shell";
  inputsFrom = [
    pkgs.shotgun
  ];
  nativeBuildInputs = [
    nixpkgs.cargo-edit
    nixpkgs.clippy
    nixpkgs.rls
    nixpkgs.rustfmt
  ];
}
