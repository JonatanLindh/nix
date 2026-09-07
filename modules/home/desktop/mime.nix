{
  xdg.mimeApps = {
    enable = true;
    defaultApplications =
      let
        browser = "zen-beta.desktop";
        image = "org.gnome.Loupe.desktop";
        fileManager = "org.gnome.Nautilus.desktop";
        video = "mpv.desktop";
        editor = "dev.zed.Zed.desktop";
        raw = "org.darktable.darktable.desktop";
      in
      {
        # Browser (Zen Beta)
        "text/html" = [ browser ];
        "x-scheme-handler/http" = [ browser ];
        "x-scheme-handler/https" = [ browser ];
        "x-scheme-handler/about" = [ browser ];
        "x-scheme-handler/unknown" = [ browser ];

        # File manager (Nautilus)
        "inode/directory" = [ fileManager ];

        # PDF (Gnome Papers)
        "application/pdf" = [ "org.gnome.Papers.desktop" ];

        # Images (Gnome Loupe)
        "image/jpeg" = [ image ];
        "image/png" = [ image ];
        "image/gif" = [ image ];
        "image/webp" = [ image ];
        "image/bmp" = [ image ];
        "image/tiff" = [ image ];
        "image/svg+xml" = [ image ];
        "image/avif" = [ image ];
        "image/heic" = [ image ];

        # RAW camera files (darktable) — DNG/generic raw also claimed by HDRMerge
        "image/x-adobe-dng" = [ raw ];
        "image/x-dcraw" = [ raw ];

        # Disk / ISO images
        "application/x-cd-image" = [ "gnome-disk-image-mounter.desktop" ];
        "application/x-raw-disk-image" = [ "gnome-disk-image-mounter.desktop" ];

        # Video (mpv)
        "video/mp4" = [ video ];
        "video/x-matroska" = [ video ];
        "video/webm" = [ video ];
        "video/quicktime" = [ video ];
        "video/x-msvideo" = [ video ];
        "video/mpeg" = [ video ];
        "video/ogg" = [ video ];

        # Audio (mpv)
        "audio/mpeg" = [ video ];
        "audio/mp4" = [ video ];
        "audio/flac" = [ video ];
        "audio/ogg" = [ video ];
        "audio/x-wav" = [ video ];
        "audio/aac" = [ video ];

        # Text / code (Zed)
        "text/plain" = [ editor ];

        # Archives — conflicts with PrismLauncher, which also claims .zip
        "application/zip" = [ fileManager ];

        # Custom scheme handlers
        "x-scheme-handler/discord" = [ "vesktop.desktop" ];
        "x-scheme-handler/spotify" = [ "spotify.desktop" ];
        "x-scheme-handler/slack" = [ "slack.desktop" ];
      };
  };
}
