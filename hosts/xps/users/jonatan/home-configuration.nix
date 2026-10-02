{ flake, ... }:
{
  imports = [
    flake.homeModules.common
    flake.homeModules.graphical
  ];
}
