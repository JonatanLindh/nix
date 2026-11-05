# Bluetooth firmware for the MediaTek MT7927 (MT6639) combo card.
#
# Linux 7.2's btmtk driver supports the chip, but linux-firmware only ships
# the WiFi half of mediatek/mt7927/. Without the BT RAM code the adapter
# fails setup, USB-resets and re-registers in a loop (~2x per second).
#
# Upstreaming (linux-firmware MR !946) is blocked on MediaTek publicly
# approving redistribution; MediaTek has hinted the blob may be OEM-only.
# !946 was closed unmerged on 2026-05-19 when the WiFi-only !1055 merged;
# follow-ups go to the linux-mediatek list. No approval as of 2026-11.
#
# So the blob is pulled from the mtkwlan.dat container in ASUS's Windows
# WiFi driver for this board (ROG STRIX X870-F GAMING WIFI). The BT blob is
# built per vendor, so take it from this board's own support page:
#   https://www.asus.com/support/api/product.asmx/GetPDDrivers?website=global&model=ROG%20STRIX%20X870-F%20GAMING%20WIFI&osid=52
# To update, bump `driver` and `hash`; the build prints the blob's build
# stamp. Drop this module once linux-firmware ships the file itself:
#   ls /run/current-system/firmware/mediatek/mt7927/  # BT_RAM_CODE_*
{ pkgs, ... }:
let
  # MTK WiFi Driver V5.7.0.6054, released 2026-07-16.
  # BT blob: build 20260611041233,
  # sha256 2135f2c4220cfa6e8eb9fdf430517098c13b862a95844bbff0153240a768efa8
  driver = "DRV_WiFi_MTK_MT7925_27_TP_W11_64_V5706054_20260706R";
  blob = "BT_RAM_CODE_MT6639_2_1_hdr.bin";

  mt7927-bt-firmware = pkgs.stdenvNoCC.mkDerivation {
    pname = "mt7927-bt-firmware";
    version = "5.7.0.6054";

    src = pkgs.fetchurl {
      name = "${driver}.zip";
      url = "https://dlcdnets.asus.com/pub/ASUS/mb/08WIRELESS/${driver}.zip";
      hash = "sha256-9CHj5up/ozXrs/tw129VBwn+L4vVJjPVGOS0K/oCGcs=";
    };

    nativeBuildInputs = [
      pkgs.unzip
      pkgs.python3
    ];

    unpackPhase = ''
      unzip -q -j $src mtkwlan.dat
    '';

    installPhase = ''
      mkdir -p $out/lib/firmware/mediatek/mt7927
      python3 -I ${./mtkwlan-extract.py} mtkwlan.dat ${blob} \
        $out/lib/firmware/mediatek/mt7927/${blob}
    '';

    meta = {
      description = "MediaTek MT7927 (MT6639) Bluetooth firmware, from ASUS's Windows driver";
      # Redistribution never cleared by MediaTek; keep it off public caches.
      license = pkgs.lib.licenses.unfree;
      platforms = pkgs.lib.platforms.linux;
    };
  };
in
{
  hardware.firmware = [ mt7927-bt-firmware ];
}
