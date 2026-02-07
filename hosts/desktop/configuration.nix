{ pkgs, flake, ... }:
{

  imports = [
    ./hardware-configuration.nix
    flake.nixosModules.common
    flake.nixosModules.desktop
  ];

  nixpkgs = {
    hostPlatform = "x86_64-linux";

    overlays = [
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
                  "HOSTLD=${llvmPkgs.lld}/bin/ld.lld"

                  "AR=${llvmPkgs.llvm}/bin/llvm-ar"
                  "NM=${llvmPkgs.llvm}/bin/llvm-nm"
                  "OBJCOPY=${llvmPkgs.llvm}/bin/llvm-objcopy"
                  "OBJDUMP=${llvmPkgs.llvm}/bin/llvm-objdump"
                  "STRIP=${llvmPkgs.llvm}/bin/llvm-strip"
                  "READELF=${llvmPkgs.llvm}/bin/llvm-readelf"

                  "KCFLAGS+=-march=znver5"
                  "KCFLAGS+=-mtune=znver5"
                  "KCFLAGS+=-O3"
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
                      X86_AMD_PSTATE_UT = module;

                      # # Topology
                      NR_CPUS = lib.mkForce (freeform "32");
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
    };
  };

  boot = {
    # kernelPackages = pkgs.linuxPackages_zen;
    kernelPackages = pkgs.linuxPackages_zen_clang;

    # # [NEW] Kernel Parameters for 9950X3D + 9070 XT
    kernelParams = [
      # Force games onto the V-Cache CCD
      "amd_x3d_vcache.mode=cache"
      "amd_pstate=active"
      "amd_pstate.shared_mem=1"

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
  };

  hardware.amdgpu = {
    initrd.enable = true;
  };

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

  networking.hostName = "desktop";

  system.stateVersion = "25.11"; # initial nixos state
}
