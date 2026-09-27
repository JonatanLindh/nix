pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire

// Who is using the microphone, the camera or the screen, read from the PipeWire graph.
// Only what goes through PipeWire is seen: apps that open /dev/video* directly (most
// browsers and video call apps today, unless their PipeWire camera option is on) use the
// camera without anything showing up here.
Singleton {
    id: root

    function cls(node) {
        return node?.properties["media.class"] ?? "";
    }

    function appName(node) {
        const p = node?.properties ?? {};
        return p["application.name"] || p["application.process.binary"] || node?.name || "";
    }

    // Streams that record without anyone being recorded: EasyEffects' own meters and
    // spectrum, level meters (Quickshell's PwNodePeakMonitor, pavucontrol) and corked
    // PulseAudio streams.
    function ignored(node) {
        const p = node.properties;
        return node.name.startsWith("ee_") || node.name.startsWith("easyeffects") || p["stream.monitor"] === "true" || p["application.name"] === "Quickshell Peak Detect" || p["pulse.corked"] === "true";
    }

    // Hardware cameras have a device; screencasts (xdg-desktop-portal-hyprland's
    // "xdph-streaming-N") are Video/Source nodes without one.
    function isScreencast(node) {
        return cls(node) === "Video/Source" && node.properties["device.id"] === undefined;
    }

    // Stream nodes of `streamClass` fed by an active link from a node `from` accepts.
    // Sink monitors belong to the Audio/Sink node, so recording what's playing doesn't count.
    function recorders(streamClass, from) {
        const found = [];
        for (const g of Pipewire.linkGroups.values) {
            const s = g.target;
            if (!s || !g.source || cls(s) !== streamClass || ignored(s) || !from(g.source))
                continue;
            if (g.state !== PwLinkState.Active || found.includes(s))
                continue;
            found.push(s);
        }
        return found;
    }

    function names(nodes) {
        return [...new Set(nodes.map(appName).filter(n => n))];
    }

    readonly property var micStreams: recorders("Stream/Input/Audio", n => cls(n).startsWith("Audio/Source"))
    readonly property var cameraStreams: recorders("Stream/Input/Video", n => cls(n) === "Video/Source" && !isScreencast(n))
    // A screencast counts while something is linked to it, i.e. someone is watching.
    readonly property var screencasts: Pipewire.nodes.values.filter(n => isScreencast(n) && Pipewire.linkGroups.values.some(g => g.source === n && g.target))

    readonly property bool mic: micStreams.length > 0
    readonly property bool camera: cameraStreams.length > 0
    readonly property bool screen: screencasts.length > 0

    readonly property var micApps: names(micStreams)
    readonly property var cameraApps: names(cameraStreams)
    // The apps receiving the screencast, not the portal producing it.
    readonly property var screenApps: names(Pipewire.linkGroups.values.filter(g => g.target && screencasts.includes(g.source)).map(g => g.target))

    // Properties (media.class, application.name, ...) and link states are only populated
    // for bound objects.
    PwObjectTracker {
        objects: Pipewire.nodes.values.concat(Pipewire.linkGroups.values)
    }
}
