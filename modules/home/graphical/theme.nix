{ lib, pkgs, ... }:
{
  home.pointerCursor = {
    enable = true;
    package = pkgs.bibata-cursors;
    name = "Bibata-Modern-Classic";

    gtk.enable = true;
    x11.enable = true;
    hyprcursor.enable = true;

    size = lib.mkDefault 20;
  };

  gtk = {
    enable = true;

    iconTheme = {
      package = pkgs.adwaita-icon-theme;
      name = "Adwaita";
    };

    font = {
      name = "Inter";
      size = 12;
    };
  };
}
