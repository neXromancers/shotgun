{ ... } @ args: let
  project = import ./nix args;
  p = project.pkgs.shotgun;
  args' = builtins.removeAttrs [ "nixpkgsSource" ] args;
in if args' == { } then p else p.override args'
