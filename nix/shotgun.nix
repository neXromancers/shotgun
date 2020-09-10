{ lib, naersk, naerskRoot ? ../., naerskSrc ? null
, cleanNaerskSource ? (path: name: path)
, libX11, libXrandr
, python3, pkgconfig
}:

naersk.buildPackage {
  root = naerskRoot;
  src = let
    cleanSourceIfPath = path:
      if builtins.isPath path then lib.cleanSource path else path;
  in if naerskSrc != null then cleanSourceIfPath naerskSrc
    else cleanNaerskSource (cleanSourceIfPath naerskRoot);

  nativeBuildInputs = [
    pkgconfig
  ];
  buildInputs = [
    libX11
    libXrandr
  ];

  meta = {
    description = "A minimal screenshot utility for X11";
    homepage = "https://github.com/neXromancers/shotgun";
    license = let l = lib.licenses; in l.mpl20;
    maintainers = let m = lib.maintainers; in [ m.bb010g ];
    platforms = let p = lib.platforms; in p.unix;
  };
}
