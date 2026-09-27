{ pkgs, ... }:

# Small helpers the quickshell config calls. `op` itself is not a runtime input: the 1Password
# app integration needs the setgid wrapper from programs._1password (/run/wrappers/bin/op).
let
  qs-paste = pkgs.writeShellApplication {
    name = "qs-paste";
    runtimeInputs = with pkgs; [ jq ];
    text = builtins.readFile ./qs-paste.sh;
  };
in
{
  home.packages = [
    qs-paste
    (pkgs.writeShellApplication {
      name = "qs-op";
      runtimeInputs = with pkgs; [
        jq
        wl-clipboard
        coreutils
        util-linux # setsid
        qs-paste
      ];
      text = builtins.readFile ./qs-op.sh;
    })
  ];
}
