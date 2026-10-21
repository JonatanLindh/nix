{ lib, pkgs, ... }:
{
  home.packages = with pkgs.kdePackages; [
    dolphin
    dolphin-plugins # git/svn status, mount ISO, …
    ark # archives (and Dolphin's compress/extract context menu)
    konsole # Dolphin's embedded terminal panel (F4)

    # KIO workers
    kio-extras # sftp, smb, mtp, archive, thumbnail, recentlyused, …
    kio-fuse # expose kio URLs (sftp://, smb://) to non-KDE apps as local paths
    kio-zeroconf # network:/ discovery

    # Thumbnails
    ffmpegthumbs # video
    kdegraphics-thumbnailers # pdf, ps, raw
    kimageformats # avif, heif, jxl, psd, …
    qtimageformats # webp, tiff
    qtsvg

    # kwalletd6, which forwards to gnome-keyring (see kwalletrc below)
    kwallet
  ];

  # Outside Plasma there is no applications.menu, so KService's sycoca stays
  # empty and Dolphin's "Open With" lists nothing.
  xdg.configFile."menus/applications.menu".text = ''
    <!DOCTYPE Menu PUBLIC "-//freedesktop//DTD Menu 1.0//EN"
      "http://www.freedesktop.org/standards/menu-spec/1.0/menu.dtd">
    <Menu>
      <Name>Applications</Name>
      <DefaultAppDirs/>
      <DefaultDirectoryDirs/>
      <Include><All/></Include>
    </Menu>
  '';

  # Store paths all have mtime 1, so sycoca never notices new or removed apps
  # after a switch. Drop it; the next KDE app rebuilds it with its own env.
  home.activation.dropKsycoca = lib.hm.dag.entryAfter [ "linkGeneration" ] ''
    run rm -f ''${XDG_CACHE_HOME:-$HOME/.cache}/ksycoca6_*
  '';

  # Written with kwriteconfig6, so the apps can still write the rest of these files.
  qt.kde.settings = {
    # Use gnome-keyring (via Secret Service) instead of KWallet's own ksecretd.
    kwalletrc.KSecretD.Enabled = false;

    kdeglobals.General = {
      TerminalApplication = "ghostty";
      TerminalService = "com.mitchellh.ghostty.desktop";
    };
  };
}
