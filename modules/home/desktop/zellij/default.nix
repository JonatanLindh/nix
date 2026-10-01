{
  programs.zellij = {
    enable = true;

    settings = {
      default_layout = "dev";
      default_mode = "locked";
      show_startup_tips = false;
    };

    # Unlock-First keybinds (Ctrl g to unlock)
    extraConfig = builtins.readFile ./keybinds.kdl;

    layouts.dev = builtins.readFile ./dev.kdl;
  };
}
