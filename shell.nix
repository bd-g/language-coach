# This sets up a development environment using nix.
# It useful to anyone who uses nix and useless otherwise.

let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  # If you want to use the beta Rust channel, use the following instead: 
  # rustChannel = nixpkgs.latest.rustChannels.beta.rust.override {
  rustChannel = nixpkgs.latest.rustChannels.stable.rust.override {
    extensions = [
      "rust-analysis"
      "rust-src"
    ];
  };
in
with nixpkgs;
mkShell {
  name = "language-coach";

  buildInputs = [
    rustChannel
    rust-analyzer
    mold-wrapped
    pkg-config
    alsa-lib
  ];

  shellHook = let
    standardLibs = [
      stdenv.cc.cc.lib
    ];
    
    standardLibPaths = lib.makeLibraryPath standardLibs;
    
  in ''
    export RUST_BACKTRACE=1;
    export LD_LIBRARY_PATH="${standardLibPaths}:$LD_LIBRARY_PATH"
    export PKG_CONFIG_PATH="${stdenv.cc.cc.lib}/lib/pkgconfig"

    export RUSTFLAGS="-C link-arg=-fuse-ld=mold"
  '';
}
