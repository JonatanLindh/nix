{ config, lib, ... }:

let
  cfg = config.idle;
  dpms = action: "hyprctl dispatch 'hl.dsp.dpms({ action = \"${action}\" })'";
  # The shader blacks out everything but the cursor (its own hardware plane),
  # so that is hidden with it.
  blankScreen =
    on:
    let
      shader = if on then "${./black.frag}" else "";
    in
    "hyprctl eval 'hl.config({ decoration = { screen_shader = \"${shader}\" }, cursor = { invisible = ${lib.boolToString on} } })'";
  unblank = blankScreen false;
in
{
  options.idle = {
    blankAfter = lib.mkOption {
      type = lib.types.nullOr lib.types.ints.positive;
      default = null;
      description = ''
        Seconds of idle before the screens show black while staying on (a
        screen shader), so they wake instantly. Null: no such stage.
      '';
    };
    screenOffAfter = lib.mkOption {
      type = lib.types.ints.positive;
      default = 330;
      description = "Seconds of idle before the screens turn off (monitor standby).";
    };
    suspendAfter = lib.mkOption {
      type = lib.types.ints.positive;
      default = 1800;
      description = "Seconds of idle before suspending.";
    };
  };

  config.services.hypridle = {
    enable = true;
    settings = {
      general = {
        lock_cmd = "pidof hyprlock || uwsm-app -- hyprlock";
        after_sleep_cmd = "${dpms "on"}; ${unblank}";
        before_sleep_cmd = "loginctl lock-session";
        # Hold off suspend until hyprlock has actually locked the session, so
        # resume lands straight on a drawn lock screen instead of the desktop
        # (or a console) while hyprlock is still starting.
        inhibit_sleep = 3;
      };

      listener = [
        {
          timeout = 150;
          on-timeout = "brightnessctl -s set 10";
          on-resume = "brightnessctl -r";
        }

        {
          timeout = 300;
          on-timeout = "loginctl lock-session";
        }
      ]
      ++ lib.optional (cfg.blankAfter != null) {
        timeout = cfg.blankAfter;
        on-timeout = blankScreen true;
        on-resume = unblank;
      }
      ++ [
        {
          timeout = cfg.screenOffAfter;
          on-timeout = dpms "off";
          on-resume = "${dpms "on"} && brightnessctl -r";
        }

        {
          timeout = cfg.suspendAfter;
          on-timeout = "systemctl suspend";
        }
      ];
    };
  };
}
