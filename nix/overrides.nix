# Fixes for locked Python packages that uv2nix cannot build from metadata alone.

final: prev: {
  # Published as an sdist only, without a declared build system.
  docopt = prev.docopt.overrideAttrs (old: {
    nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ final.resolveBuildSystem { setuptools = [ ]; };
  });
}
