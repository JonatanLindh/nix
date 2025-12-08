{ pkgs, ... }:
{
  users.users.jonatan = {
    uid = 1000;
    description = "Jonatan Lindh";

    isNormalUser = true;

    extraGroups = [
      "networkmanager"
      "wheel"
      "audio"
      "docker"
      "input"
      "libvirtd"
      "sound"
      "tty"
      "video"
      "dialout"
      "kvm"
    ];

    shell = pkgs.fish;

    # Allow to SSH from any host to any host
    # openssh.authorizedKeys.keyFiles = [ ../../authorized_keys ];
  };
}
