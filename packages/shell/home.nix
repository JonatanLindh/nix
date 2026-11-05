{ perSystem, pkgs, ... }:

# The desktop shell's home-manager side (imported by modules/home/graphical): the package from
# this folder, its user service, and the applets and programs it relies on. The shell: bars,
# panels, launcher, notifications, the drag shelf and its tools, screenshots. Hyprland binds
# reach it through `shell msg <command>` (modules/home/graphical/hyprland/lua/general.lua).
# The shelf's state is in ~/.local/state/shelf.
let
  shell = perSystem.self.shell;
in
{
  home.packages = [
    shell
  ]
  ++ (with pkgs; [
    pavucontrol # the sound panel's "Sound settings…"
    networkmanagerapplet # nm-connection-editor, the network panel's "Network settings…"
    gradia # screenshot editor (annotations, OCR), opened from the screenshot notification
    wf-recorder # screen recordings from the capture overlay
    imagemagick # crops screenshots out of the overlay's frozen frame
    libnotify # notify-send, for the screenshot notification and its actions
  ]);

  # Tray icons over StatusNotifierItem, which the bar's tray shows.
  xsession.preferStatusNotifierItems = true;

  services = {
    network-manager-applet.enable = true;
    blueman-applet.enable = true;
  };

  systemd.user.services.shell = {
    Unit = {
      Description = "Desktop shell";
      PartOf = [ "graphical-session.target" ];
      After = [ "graphical-session.target" ];
    };
    Service = {
      ExecStart = "${shell}/bin/shell";
      Restart = "on-failure";
      RestartSec = 1;
    };
    Install.WantedBy = [ "graphical-session.target" ];
  };
}
