{ ... }:

{
  services.hyprpaper = {
    enable = true;
    settings = {
      splash = false;

      wallpaper = [
        {
          monitor = "desc:Technical Concepts Ltd 27R83U X2414000812";
          path = "/home/jonatan/Pictures/Wallpapers/stream/crop1.jpg";
          fit_mode = "cover";
        }
        {
          monitor = "desc:Technical Concepts Ltd 27R83U X2414000123";
          path = "/home/jonatan/Pictures/Wallpapers/stream/crop2.jpg";
          fit_mode = "cover";
        }
        {
          monitor = "";
          path = "/home/jonatan/Pictures/Wallpapers/wallpaper.jpg";
          fit_mode = "cover";
        }
      ];
    };
  };
}
