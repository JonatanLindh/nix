{ pkgs, ... }:
{
  imports = [
    ./hyprland.nix
    ./tofi.nix
    ./mako.nix
    ./waybar.nix
    ./wleave.nix
    ./theme.nix
    ./zen.nix
    ./easyeffects
    ./mime.nix
    ./ai.nix
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
    darktable

    spotify
    google-chrome
    vesktop
    gimp3
    zed-editor
    slack
    imagemagick

    # Gaming
    mangohud
    protonup-qt

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

    ghostty = {
      enable = true;
      enableFishIntegration = true;
      settings = {
        theme = "Monokai Pro Spectrum";
        font-family = "FiraCode Nerd Font Mono";
        font-size = 15;
        background-opacity = 0.9;
      };
    };

    vscode = {
      enable = true;
      profiles.default.extensions = with pkgs.nix-vscode-extensions.vscode-marketplace; [
        ms-toolsai.jupyter
        ms-toolsai.jupyter-renderers
        marimo-team.vscode-marimo
        pkief.material-icon-theme
        ms-python.python
        mechatroner.rainbow-csv
        ms-vscode-remote.remote-ssh
        charliermarsh.ruff
      ];
    };
  };
}
