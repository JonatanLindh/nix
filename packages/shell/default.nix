# The desktop shell, as blueprint's `perSystem.self.shell` (used by ./home.nix, its
# home-manager module). The package itself is ./nix/package.nix; development happens in devenv
# (./devenv.nix).
{ pkgs, inputs, ... }:
pkgs.callPackage ./nix/package.nix { craneLib = inputs.crane.mkLib pkgs; }
