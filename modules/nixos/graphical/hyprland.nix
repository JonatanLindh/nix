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
      kdePackages.xdg-desktop-portal-kde # file chooser only
    ];
    # Replaces the hyprland-portals.conf shipped with xdph, so repeat its default.
    config.hyprland = {
      default = [
        "hyprland"
        "gtk"
      ];
      "org.freedesktop.impl.portal.FileChooser" = [ "kde" ];
    };
  };

  security.pam = {
    services = {
      hyprlock = { };
    };
  };

  environment.systemPackages = [
    pkgs.brightnessctl
    perSystem.hyprland.hyprland
  ];
}
