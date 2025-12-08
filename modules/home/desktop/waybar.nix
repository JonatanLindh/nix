{ pkgs, ... }:

{
  programs.waybar = {
    enable = true;
    systemd.enable = true;
  };

  xsession.preferStatusNotifierItems = true;

  services = {
    network-manager-applet.enable = true;
    blueman-applet.enable = true;
    playerctld.enable = true;
  };

  home.packages = with pkgs; [
    pavucontrol
  ];

}
