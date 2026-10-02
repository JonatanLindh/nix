{
  # Requires `programs.dconf.enable = true;` in system config
  services.easyeffects.enable = true;

  xdg.dataFile."easyeffects/output/DT_990.json".source = ./DT_990.json;
}
