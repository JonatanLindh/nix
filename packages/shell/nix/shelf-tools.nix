# The programs the shelf's tools run (src/services/shelf), for the package's PATH and devenv.
# Tailscale isn't here: the package puts it last on PATH, so the system's own (matching
# tailscaled) wins when there is one.
pkgs: with pkgs; [
  imagemagick
  exiftool
  (tesseract.override {
    enableLanguages = [
      "eng"
      "swe"
    ];
  })
  ghostscript
  qpdf
  ffmpeg-headless
  libarchive
  magic-wormhole
  # cp, for moving folders onto the shelf from another disk.
  coreutils
  # wl-paste, for putting the clipboard on the shelf.
  wl-clipboard
]
