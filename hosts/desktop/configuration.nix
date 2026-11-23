{
  pkgs,
  flake,
  inputs,
  config,
  ...
}:
{

  imports = [
    ./hardware-configuration.nix
    ./mt7927-bluetooth.nix
    flake.nixosModules.common
    flake.nixosModules.graphical
  ];

  nixpkgs = {
    hostPlatform = "x86_64-linux";

    overlays = [
      inputs.nix-vscode-extensions.overlays.default
      (pkgs': pkgs: {
        btop = pkgs.btop.override { rocmSupport = true; };
      })
    ];

    config = {
      rocmSupport = true;
      allowUnfree = true;
    };
  };

  boot = {
    kernelPackages = pkgs.linuxPackages_zen;

    # # [NEW] Kernel Parameters for 9950X3D + 9070 XT
    kernelParams = [
      "amd_pstate=active"

      # Disable Spectre mitigations for max gaming performance (Optional: remove if paranoid)
      "mitigations=off"
      "split_lock_detect=off"

      # RDNA 4 Power Unlocked
      "amdgpu.ppfeaturemask=0xffffffff"
    ];

    # Bootloader
    loader = {
      systemd-boot = {
        enable = true;
        configurationLimit = 5;
      };

      efi.canTouchEfiVariables = true;
    };

    extraModulePackages = [ config.boot.kernelPackages.xone ];

    tmp.useTmpfs = true;
  };

  hardware = {
    amdgpu = {
      initrd.enable = true;
    };

    graphics.extraPackages = with pkgs; [
      rocmPackages.clr
    ];

    ckb-next.enable = true;

    xone.enable = true;
  };

  # Prefer the V-Cache CCD (the driver has no kernel parameter, only sysfs)
  services.udev.extraRules = ''
    ACTION=="add|bind", SUBSYSTEM=="platform", DRIVER=="amd_x3d_vcache", ATTR{amd_x3d_mode}="cache"
  '';

  # ZRAM Swap
  zramSwap = {
    enable = true;
  };

  # Scheduler
  services.scx-loader = {
    enable = true;
    config = {
      default_sched = "scx_lavd";
      default_mode = "Auto";
      scheds.scx_lavd.auto_mode = [ "--autopilot" ];
    };
  };

  # Remote builds from the xps nix-daemon (passphrase-less key in /etc/nix/builder_ed25519),
  # restricted to the nix daemon so the key can't open a shell
  users.users.jonatan.openssh.authorizedKeys.keys = [
    "restrict,command=\"${config.nix.package}/bin/nix-daemon --stdio\" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIERzLVJj4DEbWkZrMjIyDrqITntUYAVx1n8+V7Ws1W/8 xps nix-daemon"
  ];

  networking = {
    hostName = "desktop";
    interfaces = {
      enp8s0 = {
        wakeOnLan.enable = true;
      };
    };
  };

  system.stateVersion = "25.11"; # initial nixos state
}
