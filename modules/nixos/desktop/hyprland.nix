{
  pkgs,
  inputs,
  perSystem,
  ...
}:
{
  programs.hyprland = {
    enable = true;
    withUWSM = true;
    package = inputs.hyprland.packages.${pkgs.system}.hyprland;
    portalPackage = inputs.hyprland.packages.${pkgs.system}.xdg-desktop-portal-hyprland;
  };

  xdg.portal = {
    enable = true;
    extraPortals = with pkgs; [
      xdg-desktop-portal-gtk
    ];
  };

  security.pam = {
    services = {
      hyprlock = { };
    };
  };

  environment.systemPackages = [
    perSystem.self.vulkan-hdr-layer
    pkgs.brightnessctl
  ];
}
