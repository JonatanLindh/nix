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
    flake.nixosModules.common
    flake.nixosModules.desktop
  ];

  nixpkgs = {
    hostPlatform = "x86_64-linux";

    overlays = [
      inputs.nix-vscode-extensions.overlays.default
      (
        pkgs': pkgs:
        let
          lib = pkgs'.lib;
          llvmPkgs = pkgs'.llvmPackages_latest;
        in
        {
          btop = pkgs.btop.override { rocmSupport = true; };
          linuxPackages_zen_clang = pkgs.linuxPackages_zen.extend (
            lp': lp: {
              kernel = lp.kernel.override (old: {
                stdenv = llvmPkgs.stdenv;

                nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [
                  # pkgs'.rustc
                  # pkgs'.rust-bindgen
                  # pkgs'.rustPlatform.rustLibSrc
                  llvmPkgs.lld
                  llvmPkgs.libclang
                ];

                extraMakeFlags = (old.extraMakeFlags or [ ]) ++ [
                  "LLVM=1"
                  "LLVM_IAS=1"

                  "CC=${llvmPkgs.clang-unwrapped}/bin/clang"
                  "LD=${llvmPkgs.lld}/bin/ld.lld"
                  "HOSTCC=${llvmPkgs.clang}/bin/clang"
                  "HOSTLD=${llvmPkgs.bintools}/bin/ld.lld"
                  "HOSTLDFLAGS=--ld-path=${llvmPkgs.bintools}/bin/ld.lld"

                  "AR=${llvmPkgs.llvm}/bin/llvm-ar"
                  "NM=${llvmPkgs.llvm}/bin/llvm-nm"
                  "OBJCOPY=${llvmPkgs.llvm}/bin/llvm-objcopy"
                  "OBJDUMP=${llvmPkgs.llvm}/bin/llvm-objdump"
                  "STRIP=${llvmPkgs.llvm}/bin/llvm-strip"
                  "READELF=${llvmPkgs.llvm}/bin/llvm-readelf"

                  "KCFLAGS+=-march=znver5"
                  "KCFLAGS+=-mtune=znver5"
                  "KCFLAGS+=-O2"
                  "KCFLAGS+=-pipe"
                  "KCFLAGS+=-Wframe-larger-than=4096"
                ];

                argsOverride = {
                  structuredExtraConfig =
                    with pkgs.lib.kernel;
                    (old.structuredExtraConfig or { })
                    // {
                      # # LTO
                      LTO_CLANG_THIN = yes;

                      # # Scheduling & Preemption
                      SCHED_AUTOGROUP = yes;

                      # # 9950X3D
                      AMD_3D_VCACHE = yes;

                      # # Topology
                      NR_CPUS = lib.mkForce (freeform "32");
                    }
                    // {
                      # Disable unused

                      # CPU & PLATFORM (Safe to disable Intel CPU features)
                      CPU_SUP_INTEL = pkgs.lib.mkForce no;
                      CPU_SUP_CENTAUR = pkgs.lib.mkForce no;
                      CPU_SUP_ZHAOXIN = pkgs.lib.mkForce no;

                      # Virtualization (Disable Intel VT-d, keep AMD-Vi)
                      INTEL_IOMMU = pkgs.lib.mkForce no;

                      # Disable Nvidia
                      DRM_NOUVEAU = pkgs.lib.mkForce no;

                      # Disable Intel Graphics
                      DRM_I915 = pkgs.lib.mkForce no;
                      DRM_XE = pkgs.lib.mkForce no;
                      DRM_GMA500 = pkgs.lib.mkForce no;

                      # Disable Legacy AMD (Pre-GCN)
                      DRM_RADEON = pkgs.lib.mkForce no;
                      DRM_AMDGPU_SI = pkgs.lib.mkForce no; # Southern Islands (HD 7000 series)
                      DRM_AMDGPU_CIK = pkgs.lib.mkForce no; # Sea Islands (R9 200 series)

                      # VM-guest-only display stubs (this box is a KVM host, never a guest)
                      DRM_VIRTIO_GPU = pkgs.lib.mkForce no;
                      DRM_QXL = pkgs.lib.mkForce no;
                      DRM_VMWGFX = pkgs.lib.mkForce no;
                      DRM_VBOXVIDEO = pkgs.lib.mkForce no;
                      DRM_CIRRUS_QEMU = pkgs.lib.mkForce no;
                      DRM_BOCHS = pkgs.lib.mkForce no;
                      DRM_AST = pkgs.lib.mkForce no; # server BMC framebuffer
                      DRM_MGAG200 = pkgs.lib.mkForce no; # server BMC framebuffer

                      # AUDIO & USB
                      USB4 = module;
                    };
                };

                ignoreConfigErrors = true; # Rust + LTO currently not supported
              });
            }
          );
        }
      )
    ];

    config = {
      rocmSupport = true;
      allowUnfree = true;
    };
  };

  boot = {
    # kernelPackages = pkgs.linuxPackages_zen;
    kernelPackages = pkgs.linuxPackages_zen_clang;

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
  services.scx = {
    enable = true;
    scheduler = "scx_lavd";
    extraArgs = [ "--autopilot" ];
  };

  services.sunshine = {
    enable = true;
    openFirewall = true;
  };

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
