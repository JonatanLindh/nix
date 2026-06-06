{
  pkgs,
  lib,
  inputs,
  ...
}:
{
  imports = [
    ./hyprland.nix
    ./1password.nix
    ./steam.nix
    ./flatpak.nix # For Stremio
    inputs.silentSDDM.nixosModules.default
  ];

  hardware = {
    graphics = {
      enable = true;
      enable32Bit = true;

      extraPackages = with pkgs; [
        intel-media-driver
        libva-utils
      ];
    };

    enableAllFirmware = true;
    firmware = [
      pkgs.linux-firmware
    ];

    bluetooth = {
      enable = true;
      powerOnBoot = true;
      settings = {
        General = {
          # Shows battery charge of connected devices on supported
          # Bluetooth adapters. Defaults to 'false'.
          Experimental = true;
        };
      };
    };
  };

  services.xserver.xkb = {
    layout = "se";
    variant = "";
  };

  services.displayManager.sddm = {
    enable = true;
    wayland = {
      enable = true;
      compositor = "kwin";
    };
    settings.Theme.CursorTheme = "breeze_cursors";
  };

  programs.silentSDDM = {
    enable = true;
    theme = "default";
  };

  environment.systemPackages = with pkgs; [
    kdePackages.breeze
  ];

  environment.sessionVariables = {
    NIXOS_OZONE_WL = "1";
  };

  powerManagement.resumeCommands = lib.mkAfter "${pkgs.systemd}/bin/systemctl restart tailscaled";

  # Enable networking
  networking = {
    networkmanager = {
      enable = true;
      dispatcherScripts = [
        {
          source = pkgs.writeText "99-tailscale" ''
            #!/bin/sh
            [ "$2" = "up" ] && systemctl restart tailscaled
          '';
          type = "basic";
        }
      ];
    };

    firewall = {
      enable = true;
      checkReversePath = "loose";

      allowedTCPPorts = [
        3000
        8080
        57621 # Spotify
      ];

      allowedUDPPorts = [
        5353 # Spofify
      ];

      allowedTCPPortRanges = [
        {
          from = 1714;
          to = 1764;
        } # KDE Connect
      ];

      allowedUDPPortRanges = [
        {
          from = 1714;
          to = 1764;
        } # KDE Connect
      ];
    };
  };

  virtualisation.docker = {
    enable = true;
    enableOnBoot = false;
  };
}
