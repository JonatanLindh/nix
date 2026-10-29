{
  pkgs,
  flake,
  inputs,
  ...
}:
{

  imports = [
    ./hardware-configuration.nix
    ./nvidia.nix
    ./tlp.nix
    flake.nixosModules.common
    flake.nixosModules.graphical
  ];

  nix.distributedBuilds = false;
  nix.settings.builders-use-substitutes = true;
  nix.buildMachines = [
    {
      hostName = "desktop";
      protocol = "ssh-ng";
      system = "x86_64-linux";
      sshUser = "jonatan";
      # nix-daemon runs as root without an agent, so it needs a passphrase-less key
      sshKey = "/etc/nix/builder_ed25519";
      publicHostKey = "c3NoLWVkMjU1MTkgQUFBQUMzTnphQzFsWkRJMU5URTVBQUFBSUc0QlgyNFF3NEF1RXkwRVM1cFFGTlRtb3ZIaTI5clNwT0V0MFc3NWlMNEEgcm9vdEBkZXNrdG9w";
      maxJobs = 32;
      speedFactor = 2;
      supportedFeatures = [
        "nixos-test"
        "benchmark"
        "big-parallel"
        "kvm"
      ];
    }
  ];

  boot = {
    # 7.3 + nvidia-open 615.78.08 breaks dGPU runtime D3 ("Not supported"); unpin once fixed
    kernelPackages = pkgs.linuxPackages_7_2;

    # WiFi speed is slow and crashes by default (https://bugzilla.kernel.org/show_bug.cgi?id=213381)
    # Tuning based on iwlwifi reference(https://wiki.archlinux.org/title/Network_configuration/Wireless#iwlwifi)
    extraModprobeConfig = "options iwlwifi power_save=1 11n_disable=8";

    kernelModules = [ "nvidia-uvm" ];
    kernelParams = [
      "snd_intel_dspcfg.dsp_driver=3"
      "i915.enable_fbc=1"
      "i915.enable_guc=3"
    ];

    # Bootloader
    loader = {
      systemd-boot = {
        enable = true;
        configurationLimit = 5;
      };

      efi.canTouchEfiVariables = true;
    };

    tmp.useTmpfs = true;
  };

  nixpkgs = {
    hostPlatform = "x86_64-linux";

    overlays = [
      inputs.nix-vscode-extensions.overlays.default
      (pkgs': pkgs: {
        btop = pkgs.btop.override { cudaSupport = true; };
      })
    ];

    config = {
      allowUnfree = true;
    };
  };

  networking.hostName = "xps";

  hardware.graphics.extraPackages = [ pkgs.intel-media-driver ];

  services = {
    thermald.enable = true;
    resolved.enable = true;
    upower.enable = true;
  };

  system.stateVersion = "24.05"; # initial nixos state
}
