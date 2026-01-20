{ inputs, ... }:
{

  imports = [
    inputs.self.homeModules.common
    inputs.self.homeModules.desktop
  ];
}
