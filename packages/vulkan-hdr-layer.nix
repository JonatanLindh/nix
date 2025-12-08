{
  pkgs,
  pname,
  ...
}:

let
  version = "63d2eec";
in
pkgs.stdenv.mkDerivation {
  inherit pname version;

  src =
    (pkgs.fetchFromGitHub {
      owner = "Zamundaaa";
      repo = "VK_hdr_layer";
      rev = "303e0c69e1d33acd95158d92b1fc652fb5b85399";
      fetchSubmodules = true;
      hash = "sha256-NsC44Ifl/fAHvFqP7NLrVZ71Y+x5mBEkv+r43HN5yn4=";
    }).overrideAttrs
      (_: {
        GIT_CONFIG_COUNT = 1;
        GIT_CONFIG_KEY_0 = "url.https://github.com/.insteadOf";
        GIT_CONFIG_VALUE_0 = "git@github.com:";
      });

  nativeBuildInputs = with pkgs; [
    vulkan-headers
    meson
    ninja
    pkg-config
    jq
  ];

  buildInputs = with pkgs; [
    vulkan-headers
    vulkan-loader
    vulkan-utility-libraries
    libx11
    libxrandr
    libxcb
    cmake
    wayland
    wayland-scanner
  ];

  # Help vulkan-loader find the validation layers
  setupHook = pkgs.writeText "setup-hook" ''
    addToSearchPath XDG_DATA_DIRS @out@/share
  '';

  meta = with pkgs.lib; {
    description = "Layers providing Vulkan HDR";
    homepage = "https://github.com/Zamundaaa/VK_hdr_layer";
    platforms = platforms.linux;
    license = licenses.mit;
  };
}
