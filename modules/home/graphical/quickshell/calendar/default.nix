{ pkgs, ... }:

# `qs-calendar-events`: read-only Google Calendar (iCal) events for the bar's calendar panel.
# Lives outside config/ so the hot-reloaded dir stays QML only.
{
  home.packages = [
    (pkgs.writers.writePython3Bin "qs-calendar-events" {
      libraries = with pkgs.python3Packages; [
        icalendar
        recurring-ical-events
        x-wr-timezone
      ];
      flakeIgnore = [ "E501" ];
    } (builtins.readFile ./events.py))
  ];
}
