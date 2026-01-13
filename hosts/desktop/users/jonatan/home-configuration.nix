{ pkgs, flake, ... }:
{
  imports = [
    flake.homeModules.common
    flake.homeModules.desktop
  ];

  home.packages = with pkgs; [
    blender
  ];

  # Easyeffects autoload
  xdg.dataFile."easyeffects/autoload/output/DT_990.json".text = builtins.toJSON {
    device = "alsa_output.usb-Generic_USB_Audio-00.HiFi__Headphones__sink";
    device-description = "USB Audio Front Headphones";
    device-profile = "Front Headphones";
    preset-name = "DT_990";
  };
}
