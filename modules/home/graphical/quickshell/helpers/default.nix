{ pkgs, ... }:

# The bar's data helpers, which Quickshell runs and reads JSON lines from: `qs-ts` (Tailscale
# state from tailscaled's LocalAPI, pushed on change), `qs-find` (the launcher's ranking, file
# search and calculator) and `qs-clip` (cliphist entries, previews and thumbnails). Lives
# outside config/ so the hot-reloaded dir stays QML only.
{
  home.packages = [ (pkgs.callPackage ./package.nix { }) ];
}
