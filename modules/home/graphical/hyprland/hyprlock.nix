{ ... }:

{
  programs.hyprlock = {
    enable = true;
    settings = {
      general = {
        hide_cursor = false;
        ignore_empty_input = true;
      };

      # auth = {
      #   fingerprint = {
      #     enabled = true;
      #     ready_message = "Scan fingerprint to unlock";
      #     present_message = "Scanning...";
      #     retry_delay = 250; # in milliseconds
      #   };
      # };

      animations = {
        enabled = true;
        bezier = "linear, 1, 1, 0, 0";

        animation = [
          # No fade-in: the lock screen should be fully there the moment the
          # screen comes back from sleep.
          "fadeIn, 0"
          "fadeOut, 1, 5, linear"
          "inputFieldDots, 1, 2, linear"
        ];
      };

      # The wallpapers rather than "screenshot": a screenshot needs a
      # screencopy of every output before anything is drawn, which delays
      # the lock (worst when it happens right before suspend).
      background =
        map
          (
            bg:
            {
              blur_passes = 2;
              blur_size = 6;
            }
            // bg
          )
          [
            {
              monitor = "desc:Technical Concepts Ltd 27R83U X2414000812";
              path = "/home/jonatan/Pictures/Wallpapers/stream/crop1.jpg";
            }
            {
              monitor = "desc:Technical Concepts Ltd 27R83U X2414000123";
              path = "/home/jonatan/Pictures/Wallpapers/stream/crop2.jpg";
            }
            {
              monitor = "";
              path = "/home/jonatan/Pictures/Wallpapers/wallpaper.jpg";
            }
          ];

      input-field = [
        {
          monitor = "";
          size = "20%, 5%";

          position = "0, -20";
          halign = "center";
          valign = "center";

          dots_center = true;
          fade_on_empty = false;
          font_color = "rgb(202, 211, 245)";
          inner_color = "rgb(91, 96, 120)";
          outer_color = "rgb(24, 25, 38)";
          outline_thickness = 5;
          placeholder_text = "Password...";
          shadow_passes = 2;
        }
      ];
    };
  };
}
