{
  services.tlp = {
    enable = true;
    pd.enable = true;
    settings = {
      CPU_SCALING_GOVERNOR_ON_AC = "performance";
      CPU_SCALING_GOVERNOR_ON_BAT = "powersave";
      CPU_SCALING_GOVERNOR_ON_SAV = "powersave";

      CPU_ENERGY_PERF_POLICY_ON_AC = "performance";
      CPU_ENERGY_PERF_POLICY_ON_BAT = "balance_performance";
      CPU_ENERGY_PERF_POLICY_ON_SAV = "power";

      PLATFORM_PROFILE_ON_AC = "performance";
      PLATFORM_PROFILE_ON_BAT = "balanced";
      PLATFORM_PROFILE_ON_SAV = "low-power";

      CPU_BOOST_ON_AC = 1;
      CPU_BOOST_ON_BAT = 1;
      CPU_BOOST_ON_SAV = 0;

      CPU_HWP_DYN_BOOST_ON_AC = 1;
      CPU_HWP_DYN_BOOST_ON_BAT = 1;
      CPU_HWP_DYN_BOOST_ON_SAV = 0;

      CPU_MIN_PERF_ON_AC = 0;
      CPU_MAX_PERF_ON_AC = 100;
      CPU_MIN_PERF_ON_BAT = 0;
      CPU_MAX_PERF_ON_BAT = 100;
      CPU_MIN_PERF_ON_SAV = 0;
      CPU_MAX_PERF_ON_SAV = 60;

      # Battery care
      START_CHARGE_THRESH_BAT0 = 75;
      STOP_CHARGE_THRESH_BAT0 = 80;

      # Aggressive Power Saving
      RUNTIME_PM_ON_AC = "on";
      RUNTIME_PM_ON_BAT = "auto";
      RUNTIME_PM_ON_SAV = "auto";

      PCIE_ASPM_ON_AC = "default";
      PCIE_ASPM_ON_BAT = "powersave";
      PCIE_ASPM_ON_SAV = "powersupersave";

      # Disk and Controller refinements
      SATA_LINKPWR_ON_AC = "med_power_with_dipm";
      SATA_LINKPWR_ON_BAT = "med_power_with_dipm";
      SATA_LINKPWR_ON_SAV = "min_power";

      AHCI_RUNTIME_PM_ON_AC = "on";
      AHCI_RUNTIME_PM_ON_BAT = "auto";
      AHCI_RUNTIME_PM_ON_SAV = "auto";

      DISK_APM_LEVEL_ON_AC = "254 254";
      DISK_APM_LEVEL_ON_BAT = "128 128";
      DISK_APM_LEVEL_ON_SAV = "1 1";

      # Radio and Audio
      WIFI_PWR_ON_AC = "off";
      WIFI_PWR_ON_BAT = "on";
      WIFI_PWR_ON_SAV = "on";

      SOUND_POWER_SAVE_ON_AC = 1;
      SOUND_POWER_SAVE_ON_BAT = 1;
      SOUND_POWER_SAVE_ON_SAV = 1;
    };
  };
}
