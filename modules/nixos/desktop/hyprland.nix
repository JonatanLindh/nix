{
  pkgs,
  perSystem,
  ...
}:
{
  programs.hyprland = {
    enable = true;
    withUWSM = true;
    package = perSystem.hyprland.hyprland;
    portalPackage = perSystem.hyprland.xdg-desktop-portal-hyprland;
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
    perSystem.hyprland.hyprland
  ];
}
