{
  lib,
  rustPlatform,
  makeWrapper,
  cliphist,
  wl-clipboard,
  tailscale,
}:

# `qs-ts`, `qs-find` and `qs-clip`: the bar's data helpers. The tools they drive come along on
# their PATH; tailscale goes last so the system's own (matching tailscaled) wins when there is
# one, and qs-paste comes from the user's profile (scripts/).
rustPlatform.buildRustPackage {
  pname = "qs-helpers";
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
    wrapProgram $out/bin/qs-clip --prefix PATH : ${
      lib.makeBinPath [
        cliphist
        wl-clipboard
      ]
    }
    wrapProgram $out/bin/qs-ts --suffix PATH : ${lib.makeBinPath [ tailscale ]}
  '';

  meta = {
    description = "Data helpers for the Quickshell bar: Tailscale, launcher search, clipboard history";
  };
}
