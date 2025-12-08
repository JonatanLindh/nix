{ ... }:

{
  services.mako = {
    enable = true;
    settings = {
      layer = "overlay";
      background-color = "#2e3440";
      border-color = "#88c0d0";
      border-radius = 10;
      icon-border-radius = 4;
      default-timeout = 5000;
      ignore-timeout = 1;

      "urgency=low" = {
        border-color = "#cccccc";
      };

      "urgency=normal" = {
        border-color = "#d08770";
      };

      "urgency=high" = {
        border-color = "#bf616a";
        default-timeout = 0;
      };

      "category=mpd" = {
        default-timeout = 2000;
        group-by = "category";
      };
    };
  };
}
