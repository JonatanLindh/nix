{ pkgs, ... }:

let
  jsonFormat = pkgs.formats.json { };
in
{
  home.packages = [ pkgs.wleave ];

  xdg.configFile."wleave/layout.json".source = jsonFormat.generate "wleave.json" {
    buttons = [
      {
        label = "lock";
        action = "hyprlock";
        text = "Lock";
        keybind = "l";
        icon = "${pkgs.wleave}/share/wleave/icons/lock.svg";
      }
      {
        label = "hibernate";
        action = "systemctl hibernate";
        text = "Hibernate";
        keybind = "h";
        icon = "${pkgs.wleave}/share/wleave/icons/hibernate.svg";
      }
      {
        label = "logout";
        action = "loginctl terminate-user $USER";
        text = "Logout";
        keybind = "e";
        icon = "${pkgs.wleave}/share/wleave/icons/logout.svg";
      }
      {
        label = "shutdown";
        action = "systemctl poweroff";
        text = "Shutdown";
        keybind = "s";
        icon = "${pkgs.wleave}/share/wleave/icons/shutdown.svg";
      }
      {
        label = "suspend";
        action = "systemctl suspend";
        text = "Suspend";
        keybind = "u";
        icon = "${pkgs.wleave}/share/wleave/icons/suspend.svg";
      }
      {
        label = "reboot";
        action = "systemctl reboot";
        text = "Reboot";
        keybind = "r";
        icon = "${pkgs.wleave}/share/wleave/icons/reboot.svg";
      }
    ];
    no-version-info = true;
    show-keybinds = true;
    close-on-lost-focus = true;
  };
}
