pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.Pipewire

Panel {
    id: root

    name: "vol"
    title: "vol"

    onVisibleChanged: if (visible)
        Audio.eeRefresh()

    actions: Action {
        text: Audio.muted ? "unmute" : "mute"
        onClicked: if (Audio.sink?.audio)
            Audio.sink.audio.muted = !Audio.sink.audio.muted
    }

    Meter {
        label: "output"
        value: Audio.sink?.audio?.volume ?? 0
        valueText: Audio.muted ? "mute" : Audio.volume
        fill: Audio.muted ? Theme.off : Theme.accent
        onMoved: v => Audio.setVolume(Audio.sink, v)
    }

    // e.g. "apps → easy effects → speaker"
    BarText {
        visible: Audio.filters.length > 0
        Layout.leftMargin: Theme.panelPadding
        Layout.topMargin: 10
        color: Theme.label
        text: ["apps"].concat(Audio.filters.map(n => Audio.label(n).replace(/ sink$/i, "")), [Audio.label(Audio.sink)]).join(" → ").toLowerCase()
    }

    Section {
        visible: Audio.eeRunning
        text: "easyeffects"

        Toggle {
            checked: Audio.fx
            offText: "bypass"
            onToggled: Audio.eeToggle()
        }
    }

    Repeater {
        model: Audio.eeRunning ? Audio.eePresets : []

        PanelRow {
            required property string modelData
            text: modelData
            selected: Audio.fx && modelData === Audio.eePreset
            dimmed: !Audio.fx
            onClicked: Audio.eeLoad(modelData)
        }
    }

    Section {
        text: "device"
    }

    Repeater {
        model: Audio.hardwareSinks

        PanelRow {
            required property PwNode modelData
            text: Audio.label(modelData)
            selected: modelData === Audio.sink
            onClicked: Pipewire.preferredDefaultAudioSink = modelData
        }
    }

    Meter {
        visible: Audio.source?.audio ? true : false
        label: "input"
        value: Audio.source?.audio?.volume ?? 0
        valueText: Audio.source?.audio?.muted ? "mute" : Math.round(value * 100)
        onMoved: v => Audio.setVolume(Audio.source, v)
    }

    footer: [
        Action {
            visible: Audio.eeRunning
            text: "easyeffects →"
            onClicked: {
                Quickshell.execDetached(["easyeffects"]);
                root.close();
            }
        },
        Item {
            Layout.fillWidth: true
        },
        Action {
            text: "pavucontrol →"
            onClicked: {
                Quickshell.execDetached(["pavucontrol"]);
                root.close();
            }
        }
    ]
}
