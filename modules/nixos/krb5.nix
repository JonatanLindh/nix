{ pkgs, ... }:
{
  security.krb5 = {
    enable = true;
    settings = {
      libdefaults = {
        default_realm = "CHALMERS.SE";
        default_client_realm = "CHALMERS.SE";
        default_principal = "lindhjon@CHALMERS.SE";
        dns_lookup_realm = false;
        dns_lookup_kdc = true;
        forwardable = true;
      };
      domain_realm = {
        ".chalmers.se" = "CHALMERS.SE";
        "chalmers.se" = "CHALMERS.SE";
      };
    };
  };

  environment.systemPackages = [ pkgs.krb5 ];
}
