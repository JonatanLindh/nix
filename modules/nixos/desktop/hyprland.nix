{
  pkgs,
  perSystem,
  ...
}:
{
  # programs.uwsm.waylandCompositors.hyprland.binPath = pkgs.lib.mkForce ''
  #   ${inputs.hyprland.packages.${pkgs.system}.hyprland}/bin/start-hyprland
  # '';

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
  ];
}
