{ ... }:

{
  services.hyprpaper = {
    enable = true;
    settings = {
      splash = false;

      wallpaper = [
        {
          monitor = "";
          path = "/home/jonatan/Pictures/Wallpapers/wallpaper.jpg";
          fit_mode = "cover";
        }
      ];
    };
  };
}
