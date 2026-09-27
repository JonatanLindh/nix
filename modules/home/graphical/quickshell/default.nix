{
  config,
  osConfig,
  pkgs,
  ...
}:

{
  imports = [
    ./calendar
    ./scripts
  ];

  programs.quickshell = {
    enable = true;
    systemd.enable = true;
  };

  # Symlink straight into the repo (not the store) so edits hot-reload.
  xdg.configFile."quickshell".source =
    config.lib.file.mkOutOfStoreSymlink "${osConfig.programs.nh.flake}/modules/home/graphical/quickshell/config";

  xsession.preferStatusNotifierItems = true;

  services = {
    network-manager-applet.enable = true;
    blueman-applet.enable = true;
    playerctld.enable = true;
  };

  home.packages = with pkgs; [
    pavucontrol
    kdePackages.qtdeclarative # qmlls, matching the Qt quickshell is built against
  ];
}
