pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// A glass pill (icon, 5% steps, number) at the bottom of the focused monitor for a moment
// after the volume, mute or brightness changes. Only reacts to changes of a value it has already seen, so
// startup, reloads and switching the output device stay quiet. Takes no input at all.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    // "vol" or "bri"; what is shown.
    property string kind: "vol"
    property bool shown: false
    // Changes are ignored until startup has settled, and for a moment after the sink changes
    // (a new default device, EasyEffects relinking), since those jump the volume too.
    property bool armed: false
    readonly property bool focused: Hyprland.focusedMonitor !== null && Hyprland.monitorFor(screen) === Hyprland.focusedMonitor

    readonly property bool mute: kind === "vol" && Audio.muted
    readonly property int percent: kind === "vol" ? Audio.volume : Brightness.percent

    function show(what) {
        if (!armed)
            return;
        kind = what;
        shown = true;
        hide.restart();
    }

    function settle() {
        armed = false;
        arm.restart();
    }

    visible: shown && focused
    // Anchored to the bottom edge only, so the compositor centres it horizontally.
    anchors.bottom: true
    // qmllint disable unqualified unresolved-type
    margins.bottom: 96
    // qmllint enable unqualified unresolved-type
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: box.implicitWidth
    implicitHeight: box.implicitHeight
    color: "transparent"
    mask: Region {}
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:osd"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    Component.onCompleted: settle()

    // Deferred so a volume change caused by a sink change is seen after `settle()` has run,
    // whichever of the two signals arrives first.
    Connections {
        target: Audio

        function onVolumeChanged() {
            Qt.callLater(root.show, "vol");
        }

        function onMutedChanged() {
            Qt.callLater(root.show, "vol");
        }

        function onSinkChanged() {
            root.settle();
        }
    }

    Connections {
        target: Audio.sink

        function onReadyChanged() {
            root.settle();
        }
    }

    Connections {
        target: Brightness

        function onPercentChanged() {
            // Before max_brightness has loaded the percentage is meaningless.
            if (Brightness.max > 1)
                Qt.callLater(root.show, "bri");
        }

        function onDeviceChanged() {
            root.settle();
        }
    }

    Timer {
        id: arm
        interval: 1500
        onTriggered: root.armed = true
    }

    Timer {
        id: hide
        interval: 1200
        onTriggered: root.shown = false
    }

    Glass {
        id: box

        implicitWidth: 380
        implicitHeight: 58
        radius: height / 2

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 18
            anchors.rightMargin: 20
            spacing: 14

            Icon {
                name: root.mute ? "volume-mute" : root.kind === "vol" ? "volume" : "sun"
                size: 20
                strokeWidth: 2
                color: root.mute ? Theme.textDim : Theme.text
            }

            StepMeter {
                Layout.fillWidth: true
                value: Math.max(0, Math.min(1, root.percent / 100))
                muted: root.mute
            }

            // Fixed width so the meter doesn't change length with the number.
            UiText {
                Layout.preferredWidth: 48
                horizontalAlignment: Text.AlignRight
                text: root.mute ? "Muted" : `${root.percent}`
                tabular: true
                color: root.mute ? Theme.textDim : Theme.text
                font.weight: Font.Medium
            }
        }
    }
}
