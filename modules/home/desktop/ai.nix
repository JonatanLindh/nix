{
  programs.claude-code.enable = true;
  programs.opencode = {
    enable = true;
    enableMcpIntegration = true;
    settings = {
      plugin = [
        "opencode-gemini-auth@latest"
      ];

      permission = {
        # Default to asking for everything to avoid surprises
        "*" = "ask";

        # Let it read files and use LSP freely (low risk)
        "read" = "allow";
        "lsp" = "allow";
        "glob" = "allow";

        # Block access to your .env files entirely
        "read:.env*" = "ask";
      };

      #   provider = {
      #     google = {
      #       models = {
      #         gemini-3-pro-preview = {
      #           options = {
      #             thinkingConfig = {
      #               thinkingLevel = "high";
      #               includeThoughts = true;
      #             };
      #           };
      #         };

      #         gemini-3-flash-preview = {
      #           options = {
      #             thinkingConfig = {
      #               thinkingLevel = "high";
      #               includeThoughts = true;
      #             };
      #           };
      #         };
      #       };
      #     };
      #   };
    };
  };
}
