{ pkgs, ... }:
{
  # Clipboard history store; a bar panel can list it later (`cliphist list`).
  services.cliphist = {
    enable = true;
    allowImages = true;
  };

  home.packages = with pkgs; [
    wl-clipboard # wl-copy / wl-paste
  ];
}
