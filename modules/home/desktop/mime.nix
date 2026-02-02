{
  xdg.mimeApps = {
    enable = true;
    defaultApplications =
      let
        browser = "zen-beta.desktop";
        image = "org.gnome.Loupe.desktop";
      in
      {
        # Browser (Zen Beta)
        "text/html" = [ browser ];
        "x-scheme-handler/http" = [ browser ];
        "x-scheme-handler/https" = [ browser ];
        "x-scheme-handler/about" = [ browser ];
        "x-scheme-handler/unknown" = [ browser ];

        # PDF (Gnome Papers)
        "application/pdf" = [ "org.gnome.Papers.desktop" ];

        # Images (Gnome Loupe)
        "image/jpeg" = [ image ];
        "image/png" = [ image ];
        "image/gif" = [ image ];
        "image/webp" = [ image ];

        # Custom Scheme Handlers
        "x-scheme-handler/discord" = [ "vesktop.desktop" ];
      };
  };
}
