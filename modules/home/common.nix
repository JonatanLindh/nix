{ pkgs, inputs, ... }:
{
  home.packages = with pkgs; [
    ripgrep
    fastfetch

    sccache
    rustup

    uv
    nil
    nixd

    clang-tools
    clang
  ];

  programs = {
    firefox = {
      enable = true;
    };

    git = {
      enable = true;

      settings = {
        user.name = "Jonatan Lindh";
        user.email = "jonatan.lindh1@gmail.com";

        push = {
          autoSetupRemote = true;
        };
      };

      includes = [
        { path = "${inputs.gitalias}/gitalias.txt"; }
      ];
    };

    ghostty = {
      enable = true;
      enableFishIntegration = true;
      settings = {
        theme = "Monokai Pro Spectrum";
        font-family = "FiraCode Nerd Font Mono";
        font-size = 15;
        background-opacity = 0.9;
      };
    };

    fish = {
      enable = true;
      shellAbbrs = {
        g = "git";
        sw = "nh os switch";
      };
      shellAliases = {
        zed = "zeditor";
      };
      functions = {
        o = {
          body = "uwsm-app -- xdg-open $argv &>/dev/null &; disown";
          description = "Open file detached via uwsm";
        };
      };
    };

    bash = {
      enable = true;
    };

    nix-index = {
      enable = true;
      enableFishIntegration = true;
    };

    eza = {
      enable = true;
      git = true;
      icons = "auto";
      extraOptions = [
        "--group-directories-first"
        "-l"
      ];
    };

    zoxide = {
      enable = true;
      enableNushellIntegration = true;
      enableFishIntegration = true;
      options = [ "--cmd cd" ];
    };

    starship = {
      enable = true;
      settings = { };
    };

    atuin = {
      enable = true;
      flags = [ "--disable-up-arrow" ];
    };

    direnv = {
      enable = true;
      nix-direnv.enable = true;
    };

    nh = {
      enable = true;
      clean.enable = true;
      clean.extraArgs = "--keep-since 7d --keep 5";
      flake = "/home/jonatan/nix";
    };

    gemini-cli = {
      enable = true;
    };
  };

  services = {
    udiskie.enable = true;
    ssh-agent.enable = true;
  };

  home.stateVersion = "26.05"; # initial home-manager state
}
