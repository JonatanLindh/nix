{
  pkgs,
  flake,
  inputs,
  ...
}:
{
  imports = [
    flake.homeModules.common
    flake.homeModules.graphical
    "${inputs.nixos-vscode-server}/modules/vscode-server/home.nix"
  ];

  services.vscode-server.enable = true;

  wayland.windowManager.hyprland.extraLuaFiles.host = ./host.lua;

  home.packages = with pkgs; [
    blender
    rapid-photo-downloader
    hugin
    hdrmerge
    geeqie
    qgis

    heroic
    prismlauncher
    eden
    r2modman
  ];

  # Easyeffects autoload
  xdg.dataFile."easyeffects/autoload/output/DT_990.json".text = builtins.toJSON {
    device = "alsa_output.usb-Generic_USB_Audio-00.HiFi__Headphones__sink";
    device-description = "USB Audio Front Headphones";
    device-profile = "Front Headphones";
    preset-name = "DT_990";
  };
}
