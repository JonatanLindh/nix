{
  pkgs,
  config,
  ...
}:
{
  services.xserver.videoDrivers = [ "nvidia" ];

  # TLP pins runtime PM "on" while on AC; leave the dGPU's root port alone so it can reach D3cold
  services.tlp.settings.RUNTIME_PM_DENYLIST = "00:01.0";

  hardware = {
    graphics.extraPackages = with pkgs; [
      nvidia-vaapi-driver
    ];

    nvidia = {
      open = true;
      modesetting.enable = true;
      nvidiaSettings = true;

      powerManagement = {
        enable = true;
        finegrained = true;
      };

      prime = {
        intelBusId = "PCI:0:2:0";
        nvidiaBusId = "PCI:1:0:0";

        offload = {
          enable = true;
          enableOffloadCmd = true;
        };
      };

      package = config.boot.kernelPackages.nvidiaPackages.latest;
    };
  };
}
