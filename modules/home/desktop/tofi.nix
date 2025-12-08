{ config, lib, ... }:
{
  programs.tofi = {
    enable = true;
    settings = {
      background-color = "#000A";

      width = "100%";
      height = "100%";
      padding-left = "35%";
      padding-top = "35%";
      border-width = 0;
      outline-width = 0;

      font = "monospace";

      num-results = 5;
      result-spacing = 25;
    };
  };

  home.activation.clear-tofi-cache = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
    # This command will run after a successful home-manager switch
    echo "Clearing Tofi drun cache..."
    rm -f "${config.home.homeDirectory}/.cache/tofi-drun"
  '';
}
