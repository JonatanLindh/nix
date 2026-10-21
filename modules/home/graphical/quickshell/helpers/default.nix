{ pkgs, ... }:

# The bar's data helpers, which Quickshell runs and reads JSON lines from: `qs-ts` (Tailscale
# state from tailscaled's LocalAPI, pushed on change), `qs-find` (the launcher's ranking, file
# search and qalc calculator) and `qs-clip` (cliphist entries, previews and thumbnails). Lives
# outside config/ so the hot-reloaded dir stays QML only.
{
  home.packages = [
    (pkgs.callPackage ./package.nix { })
    # The launcher's `=` answers with qalc; shift+enter opens the expression in the app, for
    # history, plots and a window that stays.
    pkgs.qalculate-qt
  ];

  # Currency conversions in both use the rates in ~/.local/share/qalculate; keep them fresh.
  systemd.user.services.qalculate-exrates = {
    Unit.Description = "Update Qalculate's exchange rates";
    Service = {
      Type = "oneshot";
      ExecStart = "${pkgs.libqalculate}/bin/qalc -e -t 1";
    };
  };
  systemd.user.timers.qalculate-exrates = {
    Unit.Description = "Update Qalculate's exchange rates daily";
    Timer = {
      OnCalendar = "daily";
      Persistent = true;
      RandomizedDelaySec = "1h";
    };
    Install.WantedBy = [ "timers.target" ];
  };
}
