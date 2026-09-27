pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Networking

Singleton {
    readonly property var devices: Networking.devices.values
    readonly property var wired: devices.filter(d => d.type === DeviceType.Wired)
    readonly property var wifi: devices.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wiredUp: wired.find(d => d.connected) ?? null
    readonly property var wifiNetwork: wifi?.networks.values.find(n => n.connected) ?? null
    readonly property bool online: wiredUp !== null || wifiNetwork !== null

    // What the bar shows after "net".
    readonly property string summary: wiredUp ? "eth" : wifiNetwork ? wifiNetwork.name.toLowerCase() : wifi && !Networking.wifiEnabled ? "off" : "down"

    function bars(strength) {
        const n = Math.max(1, Math.round(strength * 4));
        return "▂▄▆█".slice(0, n) + "_".repeat(4 - n);
    }
}
