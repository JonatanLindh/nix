{
  config,
  osConfig,
  pkgs,
  ...
}:

{
  imports = [
    ./calendar
    ./helpers
    ./scripts
    ./shelf
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
    networkmanagerapplet # nm-connection-editor, the network panel's "Network settings…"
    kdePackages.qtdeclarative # qmlls, matching the Qt quickshell is built against
    gradia # screenshot editor (annotations, OCR), opened from the screenshot notification
    wf-recorder # screen recordings from the capture overlay
    imagemagick # crops screenshots out of the overlay's frozen frame
    libnotify # notify-send, for the screenshot notification and its actions
  ];
}
