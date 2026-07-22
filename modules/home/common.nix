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

    wl-screenrec
  ];

  programs = {
    firefox = {
      enable = true;
    };

    vscode = {
      enable = true;
      profiles.default.extensions = with pkgs.nix-vscode-extensions.vscode-marketplace; [
        ms-toolsai.jupyter
        ms-toolsai.jupyter-renderers
        marimo-team.vscode-marimo
        pkief.material-icon-theme
        ms-python.python
        mechatroner.rainbow-csv
        ms-vscode-remote.remote-ssh
        charliermarsh.ruff
      ];
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

    jujutsu = {
      enable = true;
      settings = {
        user = {
          name = "Jonatan Lindh";
          email = "jonatan.lindh1@gmail.com";
        };

        ui = {
          default-command = "log";
        };

        signing = {
          behavior = "drop";
          backend = "ssh";
          key = "~/.ssh/id_ed25519.pub";
        };

        git = {
          sign-on-push = true;
        };
      };
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
      shellInitLast = ''
        fish_add_path $HOME/.cargo/bin
      '';
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

    antigravity-cli = {
      enable = true;
    };

    ssh = {
      enable = true;
      enableDefaultConfig = false;
      settings = {
        "vera" = {
          Hostname = "vera2.c3se.chalmers.se";
          User = "lindhjon";
          IdentityFile = "~/.ssh/id_ed25519";
          ForwardAgent = "yes";
        };
        "*" = {
          WarnWeakCrypto = "no-pq-kex";
          SetEnv = {
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

  home.stateVersion = "26.05";
  home.enableNixpkgsReleaseCheck = false;
}
