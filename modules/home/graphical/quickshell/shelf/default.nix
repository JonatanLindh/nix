{ pkgs, ... }:

# The drag shelf's backend: a session D-Bus service (org.jonatanlindh.Shelf) that converts,
# packs and shares files for the shelf in the bar, plus the `shelf` CLI Quickshell calls.
# Results go to ~/Downloads/Shelf. Lives outside config/ so the hot-reloaded dir stays QML only.
let
  shelf = pkgs.callPackage ./package.nix { };
in
{
  home.packages = [ shelf ];

  systemd.user.services.shelf = {
    Unit = {
      Description = "Drag shelf actions (org.jonatanlindh.Shelf)";
      PartOf = [ "graphical-session.target" ];
      After = [ "graphical-session.target" ];
    };
    Service = {
      Type = "dbus";
      BusName = "org.jonatanlindh.Shelf";
      ExecStart = "${shelf}/bin/shelf daemon";
      Restart = "on-failure";
    };
    Install.WantedBy = [ "graphical-session.target" ];
  };
}
