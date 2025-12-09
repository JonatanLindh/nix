{ pkgs, inputs, ... }:
{

  imports = [
    ./hardware-configuration.nix
    ./nvidia.nix
    inputs.self.nixosModules.common
    inputs.self.nixosModules.desktop
  ];

  nix.distributedBuilds = true;
  nix.settings.builders-use-substitutes = true;
  nix.buildMachines = [
    {
      hostName = "desktop";
      protocol = "ssh-ng";
      system = "x86_64-linux";
      sshUser = "jonatan";
      sshKey = "/home/jonatan/.ssh/id_ed25519";
      maxJobs = 48;
      speedFactor = 2;
      supportedFeatures = [
        "nixos-test"
        "benchmark"
        "big-parallel"
        "kvm"
      ];
    }
  ];

  nixpkgs = {
    overlays = [
      (pkgs': pkgs: {
        btop = pkgs.btop.override { cudaSupport = true; };
      })
    ];
  };

  boot = {
    kernelPackages = pkgs.linuxPackages_latest;

    # WiFi speed is slow and crashes by default (https://bugzilla.kernel.org/show_bug.cgi?id=213381)
    # Tuning based on iwlwifi reference(https://wiki.archlinux.org/title/Network_configuration/Wireless#iwlwifi)
    extraModprobeConfig = "options iwlwifi power_save=1 11n_disable=8 kvm.ignore_msrs=1";

    kernelModules = [ "nvidia-uvm" ];

    # Bootloader
    loader = {
      systemd-boot = {
        enable = true;
        configurationLimit = 5;
      };

      efi.canTouchEfiVariables = true;
    };
  };

  nixpkgs.hostPlatform = "x86_64-linux";

  networking.hostName = "xps";

  services = {
    thermald.enable = true;
    power-profiles-daemon.enable = true;
    resolved.enable = true;
    upower.enable = true;
  };

  system.stateVersion = "24.05"; # initial nixos state
}
