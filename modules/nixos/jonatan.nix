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
    openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGk4THpF1vNxTi0n3U1X1hj4f8HdxtDv8nhQpCbM22Qc desktop"
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAYZiCozfeIMXyScCxQY3tnayaBxk51/wsrgGZGPYxZu xps"
    ];
  };
}
