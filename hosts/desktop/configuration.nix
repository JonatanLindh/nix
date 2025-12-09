{ pkgs, inputs, ... }:
{

  imports = [
    ./hardware-configuration.nix
    inputs.self.nixosModules.common
    inputs.self.nixosModules.desktop
  ];

  boot = {
    # Use latest kernel.
    kernelPackages = pkgs.linuxPackages_latest;

    # Bootloader
    loader = {
      systemd-boot = {
        enable = true;
        configurationLimit = 5;
      };

      efi.canTouchEfiVariables = true;
    };
  };

  hardware.amdgpu = {
    initrd.enable = true;
  };

  nixpkgs.hostPlatform = "x86_64-linux";

  networking.hostName = "desktop";

  system.stateVersion = "25.11"; # initial nixos state
}
