{
  pkgs,
  lib,
  ...
}:

let
  runtimeLibs = with pkgs; [
    wayland
    libxcb
    libxkbcommon
    vulkan-loader
    fontconfig
    freetype
    dbus
    pipewire
  ];

  patchedCrates = import ./nix/patched-crates.nix { inherit (pkgs) applyPatches fetchCrate; };
in
{
  claude.code.enable = true;

  languages.rust = {
    enable = true;
    mold.enable = true;
  };

  packages =
    with pkgs;
    [
      pkg-config
      dioxus-cli
      libqalculate # qalc, for the launcher's calculator
    ]
    ++ import ./nix/shelf-tools.nix pkgs
    ++ runtimeLibs;

  env.LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
  # pipewire-sys generates its bindings with bindgen, which needs libclang.
  env.LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

  # Store files all have the same mtime, so Cargo can't see a patched crate change: when a link
  # moves to a new store path, its crate is cleaned so the next build compiles it again.
  enterShell = ''
    mkdir -p .patched
    ${lib.concatStrings (
      lib.mapAttrsToList (name: drv: ''
        if [ "$(readlink .patched/${name})" != "${drv}" ]; then
          ln -sfn ${drv} .patched/${name}
          cargo clean -p ${name} >/dev/null 2>&1 || true
        fi
      '') patchedCrates
    )}
  '';
}
