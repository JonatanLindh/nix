# The generated Hyprland Lua config of one host plus the Hyprland it runs on,
# for verify-hyprland.sh. Builds only the config files, not home-manager.
#   hypr/*.lua   the files home-manager puts in ~/.config/hypr
#   hyprland     the host's Hyprland package (bin/Hyprland, share/hypr/stubs)
{ flake, host }:
let
  system = (builtins.getFlake flake).nixosConfigurations.${host};
  inherit (system.pkgs) lib linkFarm;
  luaFiles = lib.filterAttrs (
    name: _: lib.hasPrefix "hypr/" name && lib.hasSuffix ".lua" name
  ) system.config.home-manager.users.jonatan.xdg.configFile;
in
linkFarm "hyprland-config-${host}" (
  lib.mapAttrsToList (name: file: {
    inherit name;
    path = file.source;
  }) luaFiles
  ++ [
    {
      name = "hyprland";
      path = system.config.programs.hyprland.package;
    }
  ]
)
