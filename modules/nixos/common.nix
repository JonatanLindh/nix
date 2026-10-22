{ pkgs, ... }:
{

  imports = [
    ./jonatan.nix
    ./krb5.nix
  ];

  nix = {
    settings = {
      experimental-features = [
        "nix-command"
        "flakes"
      ];
      trusted-users = [ "@wheel" ];
      cores = 0;
      max-jobs = "auto";
      download-buffer-size = 524288000;
      auto-optimise-store = true;

      extra-substituters = [
        "https://hyprland.cachix.org"
        "https://cache.nixos-cuda.org"
        "https://numtide.cachix.org"
        "https://nix-community.cachix.org"
      ];

      extra-trusted-public-keys = [
        "hyprland.cachix.org-1:a7pgxzMz7+chwVL3/pzj6jIBMioiJM7ypFP8PwtkuGc="
        "cache.nixos-cuda.org:74DUi4Ye579gUqzH4ziL9IyiJBlDpMRn9MBN8oNan9M="
        "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
        "numtide.cachix.org-1:2ps1kLBUWjxIneOy1Ik6cQjb41X0iXVXeHigGmycPPE="
      ];

    };
  };

  # Full magic SysRq (default 16 only allows sync), so REISUB works when the
  # session is wedged.
  boot.kernel.sysctl."kernel.sysrq" = 1;

  # Set your time zone.
  time.timeZone = "Europe/Stockholm";

  # Select internationalisation properties.
  i18n = {
    defaultLocale = "en_US.UTF-8";

    extraLocaleSettings = {
      LC_ADDRESS = "sv_SE.UTF-8";
      LC_IDENTIFICATION = "sv_SE.UTF-8";
      LC_MEASUREMENT = "sv_SE.UTF-8";
      LC_MONETARY = "sv_SE.UTF-8";
      LC_NAME = "sv_SE.UTF-8";
      LC_NUMERIC = "sv_SE.UTF-8";
      LC_PAPER = "sv_SE.UTF-8";
      LC_TELEPHONE = "sv_SE.UTF-8";
      LC_TIME = "sv_SE.UTF-8";
    };
  };

  # Configure console keymap
  console.keyMap = "sv-latin1";

  # Enable sound with pipewire.
  services.pulseaudio.enable = false;
  security.rtkit.enable = true;
  services.pipewire = {
    enable = true;
    alsa.enable = true;
    alsa.support32Bit = true;
    pulse.enable = true;

    extraConfig.pipewire."99-stable-quantum" = {
      "context.properties" = {
        "default.clock.quantum" = 1024;
        "default.clock.min-quantum" = 1024;
      };
    };
  };

  services = {
    fwupd.enable = true;
    printing.enable = true;
    gnome.gnome-keyring.enable = true;
    udisks2.enable = true;
    geoclue2.enable = true;
    blueman.enable = true;

    tailscale = {
      enable = true;
      # Let the user drive tailscale (set/up/down/serve/funnel) without sudo, e.g. from the bar.
      extraSetFlags = [ "--operator=jonatan" ];
    };
    openssh = {
      enable = true;
      settings = {
        PasswordAuthentication = false;
        KbdInteractiveAuthentication = false;
      };
    };

    ananicy = {
      enable = true;
      package = pkgs.ananicy-cpp;
      rulesProvider = pkgs.ananicy-rules-cachyos;
    };
  };

  # Git
  programs.git = {
    enable = true;
    package = pkgs.gitFull;
    lfs.enable = true;
    config = {
      credential.helper = "libsecret";
    };
  };

  programs.fish.enable = true;
  programs.nix-ld.enable = true;
  # Third-party scripts (e.g. Claude Code plugins' hooks) use `#!/bin/bash`; NixOS only
  # ships /bin/sh. Same bash as /bin/sh.
  systemd.tmpfiles.rules = [ "L+ /bin/bash - - - - ${pkgs.bashInteractive}/bin/bash" ];
  programs.dconf.enable = true;
  programs.localsend.enable = true;

  programs.nh = {
    enable = true;
    flake = "/home/jonatan/nix";
    clean = {
      enable = true;
      extraArgs = "--keep-since 7d --keep 5";
    };
  };

  environment.systemPackages = with pkgs; [
    btop
    fzf
    skim
    wget
    curl
    gcc
    pkg-config
    libsecret
    perf
    tldr
    just
    usbutils
    pciutils
    android-tools
    eduvpn-client
    github-cli

    # archives
    zip
    xz
    unzip
    p7zip
  ];

  fonts = {
    packages = with pkgs; [
      noto-fonts
      noto-fonts-cjk-sans
      noto-fonts-color-emoji
      corefonts
      vista-fonts
      inter
      geist-font # the Quickshell bar (Geist + Geist Mono)
      newcomputermodern # the launcher calculator's typeset results (New Computer Modern Math, as in Typst)
      nerd-fonts.symbols-only
      nerd-fonts.fira-code
      nerd-fonts.jetbrains-mono
    ];

    fontconfig = {
      enable = true;
      # Anything asking for "monospace" (the quickshell bar, ...) follows this.
      defaultFonts.monospace = [ "JetBrainsMono Nerd Font" ];
    };
  };

}
