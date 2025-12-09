{ pkgs, inputs, ... }:
{

  imports = [
    inputs.self.homeModules.common
    inputs.self.homeModules.desktop
  ];

  home.packages = with pkgs; [
    blender-hip
  ];
}
