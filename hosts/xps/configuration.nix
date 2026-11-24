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
    # Linux 7.3+ breaks nvidia-open dGPU runtime D3 ("Not supported"): kthreads
    # now start in a nullfs root, so the driver's probe can't open the PCI
    # sysfs config file. Pinned to the 6.18 LTS until the driver is fixed.
    ##CHECK: github_issue_closed(NVIDIA/open-gpu-kernel-modules, 1507)
    ## The fix must also be in the driver nixpkgs ships: compare the release
    ## that fixed it with config.hardware.nvidia.package.version (hosts/xps/
    ## nvidia.nix uses nvidiaPackages.latest). If it is, switch back to
    ## pkgs.linuxPackages_latest and drop this comment; if not, report only.
    kernelPackages = pkgs.linuxPackages_6_18;

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
