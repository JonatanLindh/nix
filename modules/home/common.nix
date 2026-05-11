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
    nixfmt

    clang-tools
    clang

    apptainer
    graphviz
  ];

  programs = {
    firefox = {
      enable = true;
    };

    vscode = {
      enable = true;
      package = pkgs.vscode.fhs;
    };

    git = {
      enable = true;

      settings = {
        user.name = "Jonatan Lindh";
        user.email = "jonatan.lindh1@gmail.com";
        user.signingkey = "~/.ssh/id_ed25519.pub";

        gpg.format = "ssh";
        commit.gpgsign = true;

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

    helix = {
      enable = true;
      settings = {
        theme = "monokai_pro_spectrum";
        editor.cursor-shape = {
          normal = "block";
          insert = "bar";
          select = "underline";
        };
      };
      languages.language = [
        {
          name = "nix";
          auto-format = true;
          formatter.command = "${pkgs.nixfmt}/bin/nixfmt";
        }
        {
          name = "rust";
          auto-format = true;
          formatter.command = "${pkgs.rustfmt}/bin/rustfmt";
        }
      ];
      themes = {
        autumn_night_transparent = {
          "inherits" = "autumn_night";
          "ui.background" = { };
        };
      };
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
      enableBashIntegration = true;
      enableFishIntegration = true;
    };

    gemini-cli = {
      enable = true;
    };

    ssh = {
      enable = true;
      matchBlocks = {
        "vera" = {
          hostname = "vera2.c3se.chalmers.se";
          user = "lindhjon";
          identityFile = "~/.ssh/id_ed25519";
          forwardAgent = true;
        };

        "*" = {
          extraOptions = {
            WarnWeakCrypto = "no-pq-kex";
          };
          setEnv = {
            TERM = "xterm-256color";
          };
        };
      };
    };
  };

  services = {
    udiskie.enable = true;
    ssh-agent.enable = true;
  };

  home.stateVersion = "26.05"; # initial home-manager state
}
