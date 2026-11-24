{
  pkgs,
  lib,
  inputs,
  ...
}:
let
  email = "jonatan.lindh1@gmail.com";

  # Keys trusted when verifying commit signatures
  allowedSigners = pkgs.writeText "allowed_signers" (
    lib.concatMapStrings (key: "${email} ${key}\n") (lib.attrValues (import ../../ssh-keys.nix))
  );
in
{
  home.packages = with pkgs; [
    ripgrep
    fd
    fastfetch

    sccache
    rustup

    uv
    nil
    nixd
    nixfmt

    clang-tools
    clang

    graphviz
  ];

  programs = {
    # ~/.cargo/config.toml; cargo itself comes from rustup
    cargo = {
      enable = true;
      package = null;
      settings.build = {
        # Intermediate artifacts per workspace under ~/.cargo/build instead of
        # in each project's target/ (which keeps only final outputs). Not one
        # shared dir: Cargo locks the whole build-dir, and `cargo clean` in
        # one project would wipe it for all.
        build-dir = "{cargo-cache-home}/build/{workspace-path-hash}";
        # Shares compiled dependencies between projects; skips incremental
        # (workspace) crates, which incremental compilation covers anyway
        rustc-wrapper = lib.getExe pkgs.sccache;
      };
    };

    git = {
      enable = true;

      settings = {
        user.name = "Jonatan Lindh";
        user.email = email;
        user.signingkey = "~/.ssh/id_ed25519.pub";

        gpg.format = "ssh";
        gpg.ssh.allowedSignersFile = "${allowedSigners}";
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
          inherit email;
        };

        ui = {
          default-command = "log";
        };

        signing = {
          behavior = "drop";
          backend = "ssh";
          key = "~/.ssh/id_ed25519.pub";
          backends.ssh.allowed-signers = "${allowedSigners}";
        };

        git = {
          sign-on-push = true;
        };
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

    devenv.enable = true;

    bash = {
      enable = true;
    };

    helix = {
      enable = true;
      defaultEditor = true;
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
      enableFishIntegration = true;
      options = [ "--cmd cd" ];
    };

    herdr = {
      enable = true;
      settings = { };
    };

    aria2 = {
      enable = true;
      settings = { };
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
      package = pkgs.openssh_gssapi;
      enableDefaultConfig = false;
      settings = {
        "vera" = {
          Hostname = "vera2.c3se.chalmers.se";
          User = "lindhjon";
          IdentityFile = "~/.ssh/id_ed25519";
          ForwardAgent = "yes";
        };
        "chalmers" = {
          Hostname = "remote12.chalmers.se";
          User = "lindhjon";
          ForwardAgent = "yes";
          GSSAPIAuthentication = "yes";
          GSSAPIDelegateCredentials = "yes";
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
  };

  home.stateVersion = "26.05";
  home.enableNixpkgsReleaseCheck = false;
}
