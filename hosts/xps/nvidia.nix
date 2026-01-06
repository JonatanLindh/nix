{
  pkgs,
  config,
  perSystem,
  ...
}:
{
  services.xserver.videoDrivers = [ "nvidia" ];

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

      package = config.boot.kernelPackages.nvidiaPackages.beta;
    };
  };

  environment.systemPackages = [
    perSystem.self.vulkan-hdr-layer
  ];
}
