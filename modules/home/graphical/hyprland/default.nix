{ pkgs, osConfig, ... }:

{
  imports = [
    ./hypridle.nix
    ./hyprlauncher.nix
    ./hyprlock.nix
    ./hyprpaper.nix
  ];

  home.packages = with pkgs; [
    hyprpicker
    hyprshot
    grim # portal screenshots (xdph)
  ];

  services.hyprpolkitagent.enable = true;

  wayland.windowManager.hyprland = {
    enable = true;
    # Follow the NixOS module's package.
    package = osConfig.programs.hyprland.package;
    # The portal comes from the NixOS module.
    portalPackage = null;
    # UWSM manages the session and graphical-session.target
    systemd.enable = false;

    # Issue restore tokens by default so Electron apps only show the share picker once
    xdph.settings.screencopy.allow_token_by_default = true;

    # Hosts can add auto-loaded files (e.g. extraLuaFiles.host) for host-only
    # settings; those load before extraConfig's require("general").
    extraLuaFiles = {
      lib = {
        content = ./lua/lib.lua;
        autoLoad = false;
      };
      general = {
        content = ./lua/general.lua;
        autoLoad = false;
      };
    };
    extraConfig = ''
      require("general")
    '';
  };
}
