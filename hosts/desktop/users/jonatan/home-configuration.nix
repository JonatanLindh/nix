{ pkgs, flake, ... }:
{
  imports = [
    flake.homeModules.common
    flake.homeModules.desktop
  ];

  home.packages = with pkgs; [
    blender
  ];
}
