pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.Pipewire

Panel {
    id: root

    readonly property var sources: Pipewire.nodes.values.filter(n => !n.isSink && !n.isStream && n.audio !== null && Audio.isHardware(n))

    // What a device is, from its PipeWire properties, as one of our icons.
    function deviceIcon(node, input) {
        const p = node?.properties ?? {};
        const s = [p["device.form-factor"], p["device.icon-name"], p["node.name"], p["node.description"]].join(" ").toLowerCase();
        if (/headset|headphone|hands-?free|bluez/.test(s))
            return "headphones";
        if (input)
            return /webcam|camera|video/.test(s) ? "camera" : "mic";
        if (/hdmi|displayport|monitor|tv/.test(s))
            return "monitor";
        return "speaker";
    }

    function launch(command) {
        Quickshell.execDetached(command);
        close();
    }

    // Only one dropdown open at a time.
    function only(dropdown) {
        for (const d of [presets, outputs, inputs])
            if (d !== dropdown)
                d.open = false;
    }

    name: "vol"
    title: "Sound"
    spacing: 12

    onVisibleChanged: {
        if (visible)
            Audio.eeRefresh();
        else
            only(null);
    }

    // Small rounded label in the signal chain.
    component Chip: Rectangle {
        property alias text: chipText.text
        property alias textColor: chipText.color
        property bool lit: false

        implicitWidth: chipText.implicitWidth + 18
        implicitHeight: 22
        radius: 11
        color: lit ? Qt.alpha(Theme.accent, 0.32) : Qt.rgba(1, 1, 1, 0.1)

        UiText {
            id: chipText
            anchors.centerIn: parent
            width: Math.min(implicitWidth, 200)
            font.pixelSize: Theme.tiny
            font.weight: Font.Medium
        }
    }

    // Chip-high, so it lines up in the Flow.
    component ChainArrow: Item {
        implicitWidth: 12
        implicitHeight: 22

        Icon {
            anchors.centerIn: parent
            name: "chevron-right"
            size: 12
            strokeWidth: 2.2
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
        }
    }

    component FooterLink: MouseArea {
        id: link

        property string text: ""

        implicitWidth: linkText.implicitWidth + 20
        implicitHeight: 32
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor

        Rectangle {
            anchors.fill: parent
            radius: Theme.rowRadius
            color: link.containsMouse ? Qt.rgba(1, 1, 1, 0.09) : "transparent"
        }
        UiText {
            id: linkText
            anchors.centerIn: parent
            text: link.text
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.8)
        }
    }

    // What's playing, with transport buttons (it used to live in the control centre).
    NowPlaying {}

    Tile {
        RowLayout {
            Layout.fillWidth: true

            UiText {
                Layout.fillWidth: true
                text: "Output"
                color: Theme.textDim
                font.pixelSize: Theme.small
                font.weight: Font.Medium
            }
            UiText {
                text: Audio.muted ? "Muted" : Audio.volume
                mono: true
                font.pixelSize: Theme.small
            }
            // Mute toggle; the icon shows the current state, like the microphone's below.
            CircleButton {
                visible: Audio.sink?.audio ? true : false
                size: 24
                iconSize: 15
                icon: Audio.muted ? "volume-mute" : "volume"
                iconColor: Audio.muted ? Theme.text : Theme.textDim
                onClicked: Audio.sink.audio.muted = !Audio.sink.audio.muted
            }
        }

        SplitSlider {
            icon: Audio.muted ? "volume-mute" : "volume-low"
            value: Audio.sink?.audio?.volume ?? 0
            fillColor: Audio.muted ? Qt.rgba(1, 1, 1, 0.35) : Theme.white
            onMoved: v => Audio.setVolume(Audio.sink, v)
        }

        // Where sound goes: apps -> filters (EasyEffects) -> the device.
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Chip {
                text: "Apps"
                textColor: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.62)
            }
            Repeater {
                model: Audio.filters

                Row {
                    id: filter

                    required property PwNode modelData

                    spacing: 6

                    ChainArrow {}
                    Chip {
                        readonly property bool ee: filter.modelData.name === "easyeffects_sink"
                        text: ee ? "EasyEffects" : Audio.label(filter.modelData).replace(/ sink$/i, "")
                        // A bypassed EasyEffects passes sound through untouched.
                        lit: !ee || Audio.fx
                    }
                }
            }
            ChainArrow {}
            Chip {
                text: Audio.label(Audio.sink)
            }
        }
    }

    Rectangle {
        visible: Audio.eeRunning
        Layout.fillWidth: true
        implicitHeight: eeColumn.implicitHeight + 12
        radius: Theme.tileRadius
        color: Theme.fill

        ColumnLayout {
            id: eeColumn
            anchors.fill: parent
            anchors.margins: 6
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                Layout.leftMargin: 10
                Layout.rightMargin: 6
                Layout.topMargin: 6
                Layout.bottomMargin: 6
                spacing: 12

                UiText {
                    Layout.fillWidth: true
                    text: "EasyEffects"
                    font.weight: Font.DemiBold
                }
                Switch {
                    checked: Audio.fx
                    onToggled: Audio.eeToggle()
                }
            }

            Dropdown {
                id: presets
                Layout.leftMargin: 6
                Layout.rightMargin: 6
                Layout.bottomMargin: 6
                label: "Preset"
                enabled: Audio.fx
                model: Audio.eePresets.map(p => ({
                            text: p
                        }))
                currentIndex: Audio.eePresets.indexOf(Audio.eePreset)
                onOpenChanged: if (open)
                    root.only(this)
                onActivated: i => Audio.eeLoad(Audio.eePresets[i])
            }
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 2

        UiText {
            Layout.leftMargin: 10
            Layout.topMargin: 2
            Layout.bottomMargin: 6
            text: "Output device"
            dim: true
            font.pixelSize: Theme.small
            font.weight: Font.Medium
        }
        Dropdown {
            id: outputs
            Layout.leftMargin: 4
            Layout.rightMargin: 4
            model: Audio.hardwareSinks.map(n => ({
                        text: Audio.label(n),
                        icon: root.deviceIcon(n, false)
                    }))
            currentIndex: Audio.hardwareSinks.indexOf(Audio.sink)
            onOpenChanged: if (open)
                root.only(this)
            onActivated: i => Pipewire.preferredDefaultAudioSink = Audio.hardwareSinks[i]
        }
    }

    ColumnLayout {
        visible: root.sources.length > 0
        Layout.fillWidth: true
        Layout.leftMargin: 4
        Layout.rightMargin: 4
        Layout.topMargin: 2
        spacing: 8

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 6
            Layout.rightMargin: 6

            UiText {
                Layout.fillWidth: true
                text: "Input"
                color: Theme.textDim
                font.pixelSize: Theme.small
                font.weight: Font.Medium
            }
            UiText {
                text: Audio.source?.audio?.muted ? "Muted" : Math.round((Audio.source?.audio?.volume ?? 0) * 100)
                mono: true
                font.pixelSize: Theme.small
            }
            // Mute the microphone, like the speaker button in the header.
            CircleButton {
                visible: Audio.source?.audio ? true : false
                size: 24
                iconSize: 15
                icon: Audio.source?.audio?.muted ? "mic-off" : "mic"
                iconColor: Audio.source?.audio?.muted ? Theme.text : Theme.textDim
                onClicked: Audio.source.audio.muted = !Audio.source.audio.muted
            }
        }
        Dropdown {
            id: inputs
            model: root.sources.map(n => ({
                        text: Audio.label(n),
                        icon: root.deviceIcon(n, true)
                    }))
            currentIndex: root.sources.indexOf(Audio.source)
            onOpenChanged: if (open)
                root.only(this)
            onActivated: i => Pipewire.preferredDefaultAudioSource = root.sources[i]
        }
        SplitSlider {
            visible: Audio.source?.audio ? true : false
            icon: Audio.source?.audio?.muted ? "mic-off" : "mic"
            value: Audio.source?.audio?.volume ?? 0
            fillColor: Audio.source?.audio?.muted ? Qt.rgba(1, 1, 1, 0.35) : Theme.white
            onMoved: v => Audio.setVolume(Audio.source, v)
        }
    }

    footer: [
        FooterLink {
            visible: Audio.eeRunning
            text: "EasyEffects…"
            onClicked: root.launch(["easyeffects"])
        },
        Item {
            Layout.fillWidth: true
        },
        FooterLink {
            text: "Sound settings…"
            onClicked: root.launch(["pavucontrol"])
        }
    ]
}
