{
  lib,
  pkgs,
  inputs,
  ...
}:
let
  # Upstream's package needs four fixes:
  # - it installs into lib/qt-6/{platformthemes,styles}, outside QT_PLUGIN_PATH;
  # - it only builds against qtbase, which compiles out the optional KDE support,
  #   so .colors schemes (KColorScheme) and KDE icon loading silently don't work;
  # - the style plugin registers as "hyprqt6engine" but only creates "hypr-style"
  #   (the name the platform theme asks for), so it never loads. Qt then falls
  #   back to whichever style it finds first; in the KDE file-chooser portal
  #   that's Union, which segfaults outside Plasma;
  # - once it does load, the style replaces the config the platform theme shares
  #   with it (libhyprqt6engine-common), leaving the theme reading freed memory.
  hyprqt6engine =
    (pkgs.callPackage "${inputs.hyprqt6engine}/nix/default.nix" { }).overrideAttrs
      (old: {
        buildInputs =
          old.buildInputs
          ++ (with pkgs.kdePackages; [
            kconfig
            kcolorscheme
            kiconthemes
            qtdeclarative # QuickControls2: org.kde.desktop style for Kirigami apps
          ]);
        postPatch = (old.postPatch or "") + ''
          substituteInPlace style/engine.json --replace-fail '"hyprqt6engine"' '"hypr-style"'
          substituteInPlace style/ProxyStyle.cpp --replace-fail \
            $'    g_pConfigManager = makeUnique<CConfigManager>();\n    g_pConfigManager->init();' \
            $'    if (!g_pConfigManager) {\n        g_pConfigManager = makeUnique<CConfigManager>();\n        g_pConfigManager->init();\n    }'
        '';
        cmakeFlags = [
          (lib.cmakeFeature "PLUGINDIR" "${placeholder "out"}/${pkgs.qt6.qtbase.qtPluginPrefix}")
        ];
      });

  kvantum = pkgs.kdePackages.qtstyleplugin-kvantum;

  # The Quickshell Glass palette (quickshell/config/Theme.qml). Kvantum takes
  # #rrggbbaa; KColorScheme can't do alpha, so `solid` is each fill over the glass.
  glass = {
    bg = "#0e1a1e";
    text = "#eef4f3";
    textDim = "#a4aeae"; # text at 68% over bg
    textFaint = "#757f80"; # text at 45%
    white = "#f4f7f6";
    ink = "#0b1d20";
    link = "#a6d9d6";
    visited = "#b79cff";
    warn = "#ff8a8a";
    neutral = "#ffa94d";
    positive = "#3ddc78";
    # Not used by the shell; only tints colour-aware icons (Breeze folders).
    accent = "#6aa3c8";
    solid = {
      view = "#112024";
      fill = "#212c30"; # white 8%
      hover = "#2b3639"; # white 12%
      active = "#394347"; # white 18%
    };
  };

  # Arc → Glass recolour of KvArcDark, plus window translucency. Hyprland's blur
  # does the frosting (Kvantum's own `blurring` only works under KWin).
  kvantumGlass =
    pkgs.runCommand "kvantum-glass"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        settings = builtins.toJSON {
          "%General" = {
            author = "Generated from KvArcDark";
            comment = "Translucent dark glass, matching the Quickshell bar";
            translucent_windows = true;
            # More solid than the shell's panels (Theme.glass is 0.62): file lists
            # need to stay readable over a busy wallpaper. Higher = more see-through.
            reduce_window_opacity = 10;
            reduce_menu_opacity = 0; # menus carry their own alpha in the SVG
            # Breeze's width. Dolphin sizes its floating status bar assuming it,
            # and Arc's 4 makes the text get elided ("1 218 file…854,2 MiB)").
            splitter_width = 1;
            blurring = false;
            popup_blurring = false;
          };
          Hacks = {
            transparent_dolphin_view = true;
            # Kvantum drops window translucency at fractional scales (both monitors
            # are 1.5x) because of a Qt drawing bug; opt back in.
            noninteger_translucency = true;
          };
          GeneralColors = {
            "window.color" = glass.bg;
            # Views (Places panel, Ark, …) sit on the glass instead of covering it.
            "base.color" = "${glass.bg}59";
            "alt.base.color" = glass.solid.fill;
            "button.color" = glass.solid.fill;
            "light.color" = glass.solid.active;
            "mid.light.color" = glass.solid.hover;
            "dark.color" = "#000000";
            "mid.color" = glass.solid.view;
            # Selections are a white wash (Theme.fillActive) under normal text.
            "highlight.color" = "#ffffff2e";
            "inactive.highlight.color" = "#ffffff1f";
            "text.color" = glass.text;
            "window.text.color" = glass.text;
            "button.text.color" = glass.text;
            "disabled.text.color" = "${glass.text}73";
            "tooltip.text.color" = glass.text;
            "highlight.text.color" = glass.text;
            "link.color" = glass.link;
            "link.visited.color" = glass.visited;
            "progress.indicator.text.color" = glass.ink;
          };
        };
        passAsFile = [ "settings" ];
      }
      ''
        python3 -I ${./kvantum-glass.py} ${kvantum}/share/Kvantum/KvArcDark $out "$settingsPath"
      '';

  # For the KDE bits that read KColorScheme rather than the style (Dolphin's
  # sidebar, message bars, …). hyprqt6engine hands this to the apps.
  glassColors =
    let
      group = bg: alt: {
        BackgroundNormal = bg;
        BackgroundAlternate = alt;
        ForegroundNormal = glass.text;
        ForegroundInactive = glass.textDim;
        ForegroundActive = glass.white;
        ForegroundLink = glass.link;
        ForegroundVisited = glass.visited;
        ForegroundNegative = glass.warn;
        ForegroundNeutral = glass.neutral;
        ForegroundPositive = glass.positive;
        DecorationFocus = glass.white;
        DecorationHover = glass.textDim;
      };
    in
    pkgs.writeText "Glass.colors" (
      lib.generators.toINI { } {
        General = {
          ColorScheme = "Glass";
          Name = "Glass";
        };
        "Colors:Window" = group glass.bg glass.solid.view;
        "Colors:View" = group glass.solid.view glass.solid.fill;
        "Colors:Button" = group glass.solid.fill glass.solid.hover;
        "Colors:Header" = group glass.bg glass.solid.view;
        "Colors:Complementary" = group glass.bg glass.solid.view;
        "Colors:Tooltip" = group glass.solid.fill glass.solid.view;
        # KColorScheme turns this background into QPalette::Accent, which is all
        # it's used for here: Kvantum paints selections with its own white wash.
        "Colors:Selection" = group glass.accent glass.solid.hover;
        "ColorEffects:Disabled" = {
          Color = glass.solid.fill;
          ColorAmount = 0;
          ColorEffect = 0;
          ContrastAmount = 0.65;
          ContrastEffect = 1;
          IntensityAmount = 0.1;
          IntensityEffect = 2;
        };
        "ColorEffects:Inactive" = {
          ChangeSelectionColor = true;
          Enable = false;
        };
        KDE.contrast = 4;
      }
    );
in
{
  qt = {
    enable = true;
    # Sets QT_QPA_PLATFORMTHEME for the shell and the systemd user session
    # (so D-Bus-activated apps like `dolphin --daemon` get it too).
    platformTheme = {
      name = "hyprqt6engine";
      package = hyprqt6engine;
    };
  };

  # KDE apps that call KStyleManager::initStyle() (Dolphin, …) ignore the
  # platform theme's style outside Plasma and fall back to Breeze unless this is set.
  qt.kde.settings.kdeglobals.KDE.widgetStyle = "kvantum";

  home.packages = [
    kvantum
    pkgs.kdePackages.breeze-icons
  ];

  xdg.configFile = {
    "Kvantum/Glass".source = kvantumGlass;
    "Kvantum/kvantum.kvconfig".text = ''
      [General]
      theme=Glass
    '';

    # Qt Widgets apps (Dolphin, Ark, …)
    "hypr/hyprqt6engine.conf".text = ''
      theme {
        color_scheme = ${glassColors}
        icon_theme = breeze-dark
        style = kvantum
        font = Geist
        font_size = 11
        font_fixed = Geist Mono
        font_fixed_size = 11
      }

      misc {
        single_click_activate = false
        menus_have_icons = true
        shortcuts_for_context_menus = true
      }
    '';

    # QML style for the hypr* Qt apps (hyprpolkitagent, hyprland-guiutils dialogs)
    "hypr/application-style.conf".text = ''
      roundness = 2
      border_width = 1
      reduce_motion = false
    '';
  };
}
