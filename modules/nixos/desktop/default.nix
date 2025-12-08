{ pkgs, ... }:
{
  imports = [
    ./hyprland.nix
    ./1password.nix
    ./steam.nix
    ./flatpak.nix # For Stremio
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

    amdgpu = {
      initrd.enable = true;
    };

    enableAllFirmware = true;

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

  services.xserver = {
    enable = true;
    dpi = 144;

    # Configure keymap in X11
    xkb = {
      layout = "se";
      variant = "";
    };
  };

  services.displayManager.gdm = {
    enable = true;
    wayland = true;
  };

  programs.seahorse.enable = true;

  environment.sessionVariables = {
    NIXOS_OZONE_WL = "1";
  };

  # Enable networking
  networking = {
    networkmanager.enable = true;

    firewall = {
      enable = true;
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

  programs.adb.enable = true;

  virtualisation.docker = {
    enable = true;
    enableOnBoot = false;
  };
}
