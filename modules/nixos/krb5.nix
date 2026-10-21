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

  # Only want kinit/klist, not Kerberos logins. Left on, this adds pam_krb5 +
  # pam_ccreds to every PAM service, and pam_ccreds' helper fork deadlocks
  # hyprlock on a wrong password (child calls exit() in a threaded process).
  security.pam.krb5.enable = false;

  environment.systemPackages = [ pkgs.krb5 ];
}
