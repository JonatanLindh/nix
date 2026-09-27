{ ... }:

{
  services.hyprpaper = {
    enable = true;
    settings = {
      splash = false;

      wallpaper = [
        {
          monitor = "";
          path = "/home/jonatan/Pictures/Wallpapers/geometrics-d.jxl";
          fit_mode = "cover";
        }
      ];
    };
  };
}
