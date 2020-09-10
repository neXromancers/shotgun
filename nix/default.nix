let
  sourcesBoot = import ./sources.nix { };
  libBoot = import (sourcesBoot.nixpkgs + "/lib");
in { ... } @ args:
libBoot.makeScope libBoot.callPackageWith (self: {
  sources = sourcesBoot;
  nixpkgsConfig = { };
  nixpkgsOverlays = [
    (nixpkgs: nixpkgsSuper: {
      gitignoreSrc = import self.sources."gitignore.nix" { lib = libBoot; };
      naersk = nixpkgs.callPackage self.sources.naersk { };
      niv = (import self.sources.niv { pkgs = nixpkgs; }).niv;
    })
  ];
  nixpkgsCrossOverlays = [ ];
  nixpkgsArg = {
    config = self.nixpkgsConfig;
    overlays = self.nixpkgsOverlays;
    crossOverlays = self.nixpkgsCrossOverlays;
  };
  nixpkgsSource = args.nixpkgsSource or "nixpkgs";
  nixpkgsFun = import self.sources.${self.nixpkgsSource};
  nixpkgs = self.nixpkgsFun self.nixpkgsArg;
  lib = self.nixpkgs.lib.extend (lib: libSuper: {
    makeRelativeFilter = let
      inherit (builtins) stringLength substring;
    in filter: root: let
      root' = toString root + "/";
      rootLength' = stringLength root';
    in path: type: let
      isRootChild = substring 0 rootLength' path == root';
      relativePath = substring rootLength' (-1) path;
    in isRootChild -> filter relativePath type;
    cleanNixRootFilter = lib.makeRelativeFilter (relPath: type: let
      inherit (builtins) any;
      inherit (lib) hasPrefix;
    in !(any (p: relPath == p) [
        "default.nix"
        "nix"
        "shell.nix"
      ] || any (p: hasPrefix p relPath) [
        "nix/"
      ]
    ));
    cleanNixRootSource = src: lib.cleanSourceWith
      { filter = lib.cleanNixRootFilter src; inherit src; };
  });

  pkgs = self.lib.makeScope (scope: self.nixpkgs.newScope ({
    inherit (self) lib nixpkgs sources pkgs;
  } // scope)) (import ./pkgs.nix);
})
