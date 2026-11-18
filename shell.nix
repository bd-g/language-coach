# Development shell. Python dependencies come from uv.lock through uv2nix, so
# `uv lock` stays the single source of truth for package versions.

let
  sources = import ./nix/sources.nix;
  pkgs = import sources.nixpkgs { };
  inherit (pkgs) lib;

  pyproject-nix = import sources.pyproject-nix { inherit lib; };
  uv2nix = import sources.uv2nix { inherit pyproject-nix lib; };
  pyproject-build-systems = import sources.pyproject-build-systems {
    inherit pyproject-nix uv2nix lib;
  };

  python = pkgs.python313;

  workspace = uv2nix.lib.workspace.loadWorkspace { workspaceRoot = ./.; };

  pythonSet =
    (pkgs.callPackage pyproject-nix.build.packages { inherit python; }).overrideScope
      (
        lib.composeManyExtensions [
          pyproject-build-systems.overlays.wheel
          (workspace.mkPyprojectOverlay { sourcePreference = "wheel"; })
          (import ./nix/overrides.nix)
        ]
      );

  virtualenv = pythonSet.mkVirtualEnv "language-coach-dev-env" workspace.deps.all;
in
pkgs.mkShell {
  name = "language-coach";

  packages = [
    virtualenv
    pkgs.uv
    pkgs.pyright
  ];

  env = {
    UV_NO_SYNC = "1";
    UV_PYTHON = "${virtualenv}/bin/python";
    UV_PYTHON_DOWNLOADS = "never";
    MEM0_TELEMETRY = "False";
  };

  shellHook = ''
    unset PYTHONPATH
  '';
}
