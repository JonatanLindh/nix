{
  pkgs,
  perSystem,
  ...
}:
{
  programs.hyprland = {
    enable = true;
    withUWSM = true;
    package = perSystem.hyprland.hyprland;
    portalPackage = perSystem.hyprland.xdg-desktop-portal-hyprland;
  };

  services.displayManager.sessionPackages = [
    (pkgs.runCommand "hyprland-gdm-fix"
      {
        passthru.providedSessions = [
          "hyprland-fixed"
          "hyprland-uwsm-fixed"
        ];
      }
      ''
        mkdir -p $out/share/wayland-sessions

        # 1. Copy and Patch UWSM version
        cp ${perSystem.hyprland.hyprland}/share/wayland-sessions/hyprland-uwsm.desktop \
           $out/share/wayland-sessions/hyprland-uwsm-fixed.desktop

        # Fix Validation
        sed -i 's/^DesktopNames=/X-DesktopNames=/' $out/share/wayland-sessions/hyprland-uwsm-fixed.desktop
        # Change Name
        sed -i 's/^Name=.*/Name=Hyprland (UWSM-Fixed)/' $out/share/wayland-sessions/hyprland-uwsm-fixed.desktop

        # FORCE ABSOLUTE PATHS FOR GDM
        # Replace 'Exec=uwsm' with 'Exec=/nix/store/.../bin/uwsm'
        sed -i "s#Exec=uwsm#Exec=${pkgs.uwsm}/bin/uwsm#" $out/share/wayland-sessions/hyprland-uwsm-fixed.desktop
        sed -i "s#TryExec=uwsm#TryExec=${pkgs.uwsm}/bin/uwsm#" $out/share/wayland-sessions/hyprland-uwsm-fixed.desktop

        # 2. Copy and Patch Vanilla version
        cp ${perSystem.hyprland.hyprland}/share/wayland-sessions/hyprland.desktop \
           $out/share/wayland-sessions/hyprland-fixed.desktop
        sed -i 's/^DesktopNames=/X-DesktopNames=/' $out/share/wayland-sessions/hyprland-fixed.desktop
        sed -i 's/^Name=.*/Name=Hyprland (Fixed)/' $out/share/wayland-sessions/hyprland-fixed.desktop
      ''
    )
  ];

  xdg.portal = {
    enable = true;
    extraPortals = with pkgs; [
      xdg-desktop-portal-gtk
    ];
  };

  security.pam = {
    services = {
      hyprlock = { };
    };
  };

  environment.systemPackages = [
    perSystem.self.vulkan-hdr-layer
    pkgs.brightnessctl
  ];
}
