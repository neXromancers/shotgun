pkgs: let
  inherit (pkgs.callPackage ({ lib, nixpkgs, sources } @ args: args) { })
    lib nixpkgs sources;
in {
  cleanNaerskFilter = src: let
    src' = if builtins.isPath src then src else src.origSrc;
    gitignoreFilter = nixpkgs.gitignoreSrc.gitignoreFilter src';
    nixRootFilter = lib.cleanNixRootFilter src';
  in path: type: gitignoreFilter path type && nixRootFilter path type;
  cleanNaerskSource = src:
    lib.cleanSourceWith { filter = pkgs.cleanNaerskFilter src; inherit src; };

  shotgun = pkgs.callPackage ./shotgun.nix { };
}
