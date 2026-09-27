pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire

Singleton {
    id: root

    // Hardware sinks/sources have a device.id; virtual ones (EasyEffects, filter-chains) don't.
    function isHardware(node) {
        return node?.properties["device.id"] !== undefined;
    }

    function isDeviceSink(node) {
        return node.isSink && !node.isStream && node.audio !== null;
    }

    // The hardware sink a virtual sink ends up in: follow links out of its node.group.
    function downstreamOf(node) {
        const group = node?.properties["node.group"];
        if (!group)
            return null;
        const link = Pipewire.links.values.find(l => l.target && l.target.isSink && isHardware(l.target) && l.source?.properties["node.group"] === group);
        return link?.target ?? null;
    }

    function label(node) {
        return node ? (node.nickname || node.description || node.name) : "";
    }

    function setVolume(node, v) {
        if (!node?.audio)
            return;
        node.audio.muted = false;
        node.audio.volume = Math.max(0, Math.min(1, v));
    }

    readonly property var hardwareSinks: Pipewire.nodes.values.filter(n => isDeviceSink(n) && isHardware(n))

    // The device sound actually ends up on; volume always applies here.
    readonly property PwNode sink: {
        const d = Pipewire.defaultAudioSink;
        return !d || isHardware(d) ? d : (downstreamOf(d) ?? d);
    }
    readonly property PwNode source: Pipewire.defaultAudioSource
    readonly property bool muted: sink?.audio?.muted ?? false
    readonly property int volume: Math.round((sink?.audio?.volume ?? 0) * 100)

    // Virtual sinks that feed into `sink`, e.g. EasyEffects. EasyEffects unlinks its chain
    // while nothing plays, so an unlinked virtual sink is assumed to feed the current device.
    readonly property var filters: Pipewire.nodes.values.filter(n => {
        if (!isDeviceSink(n) || isHardware(n))
            return false;
        const target = downstreamOf(n);
        return target === sink || target === null;
    })

    // Properties (device.id, node.group, ...) are only populated for bound nodes.
    PwObjectTracker {
        objects: Pipewire.nodes.values
    }

    // EasyEffects: no D-Bus API, but its CLI talks to the running instance. Careful: with no
    // instance running, the CLI starts a full new EasyEffects instead of failing, so only call
    // it while the service is active and its socket exists (not while it's shutting down).
    function eeCli(args) {
        return ["sh", "-c", 'systemctl --user is-active --quiet easyeffects.service' + ' && [ -S "$XDG_RUNTIME_DIR/EasyEffectsServer" ] && exec easyeffects "$@"', "sh"].concat(args);
    }

    readonly property bool eeRunning: Pipewire.nodes.values.some(n => n.name === "easyeffects_sink")
    property bool eeBypass: false
    readonly property bool fx: eeRunning && !eeBypass
    property var eePresets: []
    property string eePreset: ""

    function eeToggle() {
        eeBypass = !eeBypass;
        eeCommand.command = eeCli(["--bypass-toggle"]);
        eeCommand.running = true;
    }

    function eeLoad(name) {
        eePreset = name;
        eeCommand.command = eeCli(["-l", name]);
        eeCommand.running = true;
    }

    function eeRefresh() {
        if (!eeRunning)
            return;
        eeBypassQuery.running = true;
        eePresetsQuery.running = true;
        eeLastQuery.running = true;
    }

    Process {
        id: eeCommand
        onRunningChanged: if (!running)
            root.eeRefresh()
    }

    Process {
        id: eeBypassQuery
        command: root.eeCli(["-b", "3"])
        stdout: StdioCollector {
            onStreamFinished: root.eeBypass = text.trim() === "1"
        }
    }

    Process {
        id: eePresetsQuery
        command: root.eeCli(["-p"])
        stdout: StdioCollector {
            // "Output presets:\n1\tName\n...\n\nInput presets..."
            onStreamFinished: {
                const out = text.split("Output presets:")[1]?.split(/\n\s*\n/)[0] ?? "";
                root.eePresets = out.split("\n").map(l => l.replace(/^\s*\d+\t/, "").trim()).filter(l => l);
            }
        }
    }

    Process {
        id: eeLastQuery
        command: root.eeCli(["-s"])
        stdout: StdioCollector {
            onStreamFinished: root.eePreset = (text.match(/output:\s*(.*)/)?.[1] ?? "").trim()
        }
    }

    // Bypass state lives only in EasyEffects' memory, so poll it (cheap, ~40ms of CPU).
    Timer {
        interval: 5000
        running: root.eeRunning
        repeat: true
        triggeredOnStart: true
        onTriggered: eeBypassQuery.running = true
    }

    onEeRunningChanged: eeRefresh()
}
