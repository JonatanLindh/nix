{
  lib,
  rustPlatform,
  makeWrapper,
  imagemagick,
  exiftool,
  tesseract,
  ghostscript,
  qpdf,
  ffmpeg-headless,
  libarchive,
  magic-wormhole,
  tailscale,
  coreutils,
  wl-clipboard,
}:

# `shelf`: the drag shelf's daemon and CLI. The tools it drives come along on its PATH;
# tailscale goes last so the system's own (matching tailscaled) wins when there is one.
rustPlatform.buildRustPackage {
  pname = "shelf";
  version = "0.1.0";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./Cargo.toml
      ./Cargo.lock
      ./src
    ];
  };
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram $out/bin/shelf \
      --prefix PATH : ${
        lib.makeBinPath [
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
          # wl-paste, for `shelf paste`.
          wl-clipboard
        ]
      } \
      --suffix PATH : ${lib.makeBinPath [ tailscale ]}

    # D-Bus activation, so the first call starts the daemon if the service isn't up yet.
    mkdir -p $out/share/dbus-1/services
    cat > $out/share/dbus-1/services/org.jonatanlindh.Shelf.service <<SERVICE
    [D-BUS Service]
    Name=org.jonatanlindh.Shelf
    Exec=$out/bin/shelf daemon
    SystemdService=shelf.service
    SERVICE
  '';

  meta = {
    description = "Actions for the Quickshell drag shelf: convert, share and send files";
    mainProgram = "shelf";
  };
}
