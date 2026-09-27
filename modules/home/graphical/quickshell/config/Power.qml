pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower

Singleton {
    id: root

    // Quickshell can't tell whether a profiles daemon exists (the desktop has none).
    property bool profilesAvailable: false
    readonly property var profiles: PowerProfiles.hasPerformanceProfile ? [PowerProfile.PowerSaver, PowerProfile.Balanced, PowerProfile.Performance] : [PowerProfile.PowerSaver, PowerProfile.Balanced]

    readonly property UPowerDevice battery: UPower.displayDevice
    readonly property bool hasBattery: battery?.isLaptopBattery ?? false
    readonly property int percent: Math.round((battery?.percentage ?? 0) * 100)
    readonly property bool charging: battery?.state === UPowerDeviceState.Charging || battery?.state === UPowerDeviceState.PendingCharge
    readonly property bool onAc: charging || battery?.state === UPowerDeviceState.FullyCharged || battery?.state === UPowerDeviceState.PendingDischarge
    readonly property bool low: hasBattery && !onAc && percent <= 15
    readonly property string timeLeft: {
        const s = charging ? battery?.timeToFull : battery?.timeToEmpty;
        if (!s)
            return "";
        const h = Math.floor(s / 3600), m = Math.round((s % 3600) / 60);
        return (h > 0 ? h + "h " : "") + m + "m " + (charging ? "to full" : "left");
    }

    function shortName(p) {
        return p === PowerProfile.Performance ? "perf" : p === PowerProfile.PowerSaver ? "save" : "bal";
    }

    function longName(p) {
        return p === PowerProfile.Performance ? "performance" : p === PowerProfile.PowerSaver ? "power-saver" : "balanced";
    }

    // Profile to show. Rapid clicks only move `pending`; the daemon gets the final choice once
    // clicks stop, since tlp-pd applies (and reports back) every profile it is sent, in order.
    property int pending: -1
    readonly property int profile: pending >= 0 ? pending : PowerProfiles.profile

    function setProfile(p) {
        pending = p;
        apply.restart();
    }

    function cycle() {
        const i = profiles.indexOf(profile);
        setProfile(profiles[(i + 1) % profiles.length]);
    }

    Timer {
        id: apply
        interval: 400
        onTriggered: {
            PowerProfiles.profile = root.pending;
            settle.restart();
        }
    }

    // Keep showing the choice until the daemon has caught up.
    Timer {
        id: settle
        interval: 3000
        onTriggered: root.pending = -1
    }

    property bool canHibernate: false

    Process {
        running: true
        command: ["busctl", "call", "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "CanHibernate"]
        stdout: StdioCollector {
            onStreamFinished: root.canHibernate = text.trim() === 's "yes"'
        }
    }

    Process {
        running: true
        command: ["sh", "-c", "busctl --system status net.hadess.PowerProfiles >/dev/null 2>&1" + " || busctl --system status org.freedesktop.UPower.PowerProfiles >/dev/null 2>&1" + " && echo yes"]
        stdout: StdioCollector {
            onStreamFinished: root.profilesAvailable = text.trim() === "yes"
        }
    }
}
