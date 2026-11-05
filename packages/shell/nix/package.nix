{
  lib,
  craneLib,
  rustPlatform,
  applyPatches,
  fetchCrate,
  pkg-config,
  makeWrapper,
  libqalculate,
  tailscale,
  pkgs,
  wayland,
  libxcb,
  libxkbcommon,
  vulkan-loader,
  fontconfig,
  freetype,
  dbus,
  pipewire,
}:

# Built with crane in two derivations: the dependencies (from Cargo.toml and Cargo.lock alone),
# then the shell on top of their compiled artifacts. A change to the shell's own sources only
# rebuilds the second; the dependencies build again only when the lock file or a patched crate
# changes.
let
  patchedCrates = import ./patched-crates.nix { inherit applyPatches fetchCrate; };

  # gpui opens these with dlopen, so they go on the binary's rpath rather than only being linked.
  runtimeLibs = [
    wayland
    libxcb
    libxkbcommon
    vulkan-loader
  ];

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../assets
    ];
  };

  commonArgs = {
    inherit src;
    strictDeps = true;

    # Where Cargo.toml's [patch.crates-io] looks for the patched gpui crates; both derivations
    # need them.
    postPatch = ''
      mkdir -p .patched
      ${lib.concatStrings (
        lib.mapAttrsToList (name: drv: ''
          ln -sfn ${drv} .patched/${name}
        '') patchedCrates
      )}
    '';

    nativeBuildInputs = [
      pkg-config
      # pipewire-sys generates its bindings with bindgen, which needs libclang.
      rustPlatform.bindgenHook
    ];

    buildInputs = runtimeLibs ++ [
      fontconfig
      freetype
      dbus
      pipewire
    ];
  };

  # Named "shell-deps" by crane.
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    # The tests run in development (cargo test); the package only needs the binary.
    doCheck = false;

    nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ makeWrapper ];

    # qalc for the launcher's calculator, and what the shelf's tools run (./shelf-tools.nix).
    postFixup = ''
      patchelf --add-rpath ${lib.makeLibraryPath runtimeLibs} $out/bin/shell
      wrapProgram $out/bin/shell \
        --prefix PATH : ${lib.makeBinPath ([ libqalculate ] ++ import ./shelf-tools.nix pkgs)} \
        --suffix PATH : ${lib.makeBinPath [ tailscale ]}
    '';

    meta = {
      description = "Hyprland desktop shell: bar, panels, launcher, notifications, shelf";
      mainProgram = "shell";
      platforms = lib.platforms.linux;
    };
  }
)
