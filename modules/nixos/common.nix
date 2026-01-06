{ pkgs, lib, ... }:
{

  imports = [
    ./jonatan.nix
    # inputs.home-manager.nixosModules.default
    # inputs.srvos.nixosModules.common
    # inputs.srvos.nixosModules.mixins-nix-experimental
    # inputs.srvos.nixosModules.mixins-terminfo
  ];

  nix = {
    settings = {
      experimental-features = [
        "nix-command"
        "flakes"
      ];
      trusted-users = [ "@wheel" ];
      cores = 0;
      max-jobs = 48;
      download-buffer-size = 524288000;
      auto-optimise-store = true;

      substituters = lib.mkOverride 1000 [
        "https://hetzner-cache.numtide.com"
        "https://numtide.cachix.org"
        "https://cuda-maintainers.cachix.org"
        "https://hyprland.cachix.org"
        "https://nix-community.cachix.org"
      ];

      trusted-public-keys = lib.mkOverride 1000 [
        "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspc6rC48="
        "numtide.cachix.org-1:2ps1kLBUWjxIneOy1Ik6cQjb41X0iXVXeHigGmycPPE="
        "cuda-maintainers.cachix.org-1:0dq3bujKpuEPMCX6U4WylrUDZ9JyUG0VpVZa7CNfq5E="
        "hyprland.cachix.org-1:a7pgxzMz7+chwVL3/pzj6jIBMioiJM7ypFP8PwtkuGc="
        "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
      ];

    };
  };

  nixpkgs.config.allowUnfree = true;

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
    # If you want to use JACK applications, uncomment this
    # jack.enable = true;

    # use the example session manager (no others are packaged yet so
    # this is enabled by default, no need to redefine it in your config
    # for now)
    #media-session.enable = true;
  };

  services = {
    fwupd.enable = true;
    printing.enable = true;
    gnome.gnome-keyring.enable = true;
    udisks2.enable = true;
    blueman.enable = true;
    geoclue2.enable = true;

    tailscale.enable = true;
    openssh = {
      enable = true;
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
  programs.dconf.enable = true;

  environment.systemPackages = with pkgs; [
    btop
    fzf
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
      nerd-fonts.symbols-only
      nerd-fonts.fira-code
    ];

    fontconfig = {
      enable = true;
    };
  };

}
