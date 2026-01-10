{ pkgs, perSystem, ... }:
{
  imports = [
    ./hyprland.nix
    ./tofi.nix
    ./mako.nix
    ./waybar.nix
    ./wleave.nix
    ./theme.nix
    ./zen.nix
  ];

  home.packages = with pkgs; [
    # Gnome
    baobab # disk usage analyzer
    resources
    nautilus # file manager
    gnome-disk-utility
    gnome-font-viewer
    loupe # image viewer
    gnome-logs
    papers

    spotify
    google-chrome
    vesktop
    gimp3
    perSystem.nixpkgs-zed.zed-editor

    # Gaming
    mangohud
    heroic
    protonup-qt
    prismlauncher

    # Typst
    typst
    tinymist
    typstyle
  ];

  programs = {
    mpv = {
      enable = true;
      config = {
        vo = "gpu-next";
        profile = "gpu-hq";
        gpu-api = "vulkan";
        gpu-context = "waylandvk";
        target-colorspace-hint = "auto";
      };
    };
  };
}
