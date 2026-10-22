pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Services.UPower
import Quickshell.Wayland

// qmllint disable uncreatable-type
PanelWindow {
    id: bar

    // Name of the open panel ("" when none), and the panel it was opened from, if any
    // (e.g. Tailscale opened from the network panel shows a back arrow to "net").
    property string panel: ""
    property string panelFrom: ""

    function toggle(name) {
        panelFrom = "";
        panel = panel === name ? "" : name;
    }

    function open(name, from) {
        panelFrom = from ?? "";
        panel = name;
    }

    anchors {
        top: true
        left: true
        right: true
    }
    implicitHeight: Theme.barHeight
    color: Theme.barBg
    WlrLayershell.namespace: "quickshell:bar"
    // Panels are popups of this layer surface; without keyboard focus here their text
    // fields (Wi-Fi password, tailscale serve) can't be typed into.
    // Clip and the launcher are their own windows and take the keyboard themselves.
    WlrLayershell.keyboardFocus: panel !== "" && !["clip", "launcher", "notif", "session", "op"].includes(panel) ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.None

    IdleInhibitor {
        window: bar
        enabled: Caf.enabled
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: Theme.barPadding
        height: parent.height
        spacing: Theme.gap

        ShelfButton {
            id: shelf
            anchors.verticalCenter: parent.verticalCenter
            bar: bar
        }

        PickerSlot {
            id: picker
            anchors.verticalCenter: parent.verticalCenter
            bar: bar
        }

        Separator {
            visible: shelf.visible || picker.visible
        }

        Workspaces {
            anchors.verticalCenter: parent.verticalCenter
            screen: bar.screen
        }

        // Last, since it grows and shrinks as apps come and go.
        Separator {
            visible: tray.visible
        }

        Tray {
            id: tray
            anchors.verticalCenter: parent.verticalCenter
            bar: bar
        }
    }

    // Media used to hang off the clock; now playing lives in the control centre.
    Clock {
        anchors.centerIn: parent
        active: bar.panel === "cal"
        onClicked: bar.toggle("cal")
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.barPadding
        height: parent.height
        spacing: 2

        // While a screen recording runs: red dot and elapsed time; click stops it.
        IconButton {
            id: recording
            visible: Screenshot.recording
            anchors.verticalCenter: parent.verticalCenter
            restColor: Qt.alpha(Theme.warn, 0.22)
            onClicked: Screenshot.stop()

            property int seconds: 0
            Timer {
                running: Screenshot.recording
                interval: 1000
                repeat: true
                triggeredOnStart: true
                onTriggered: recording.seconds = Math.floor((Date.now() - Screenshot.recordingSince.getTime()) / 1000)
            }

            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 9
                height: 9
                radius: 4.5
                color: Theme.warn
            }
            UiText {
                anchors.verticalCenter: parent.verticalCenter
                text: `${Math.floor(recording.seconds / 60)}:${String(recording.seconds % 60).padStart(2, "0")}`
                tabular: true
            }
        }

        // Only while something records the mic, camera or screen.
        PrivacyIndicators {
            id: privacy
        }

        Item {
            visible: privacy.visible
            implicitWidth: 4
        }

        // Only off Balanced; a click goes back to it.
        IconButton {
            visible: Power.profilesAvailable && Power.profile !== PowerProfile.Balanced
            anchors.verticalCenter: parent.verticalCenter
            icon: Power.profile === PowerProfile.PowerSaver ? "leaf" : "bolt"
            iconSize: 16
            iconColor: Theme.accent
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton)
                    bar.toggle("pwr");
                else
                    Power.setProfile(PowerProfile.Balanced);
            }
        }

        // Only while a funnel makes something reachable from the public internet; the
        // rest of tailscale lives in the control centre.
        IconButton {
            visible: Ts.funnelActive
            anchors.verticalCenter: parent.verticalCenter
            icon: "tailscale"
            iconSize: 16
            iconColor: Theme.accent
            active: bar.panel === "ts"
            onClicked: bar.toggle("ts")
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: Audio.muted || Audio.volume === 0 ? "volume-mute" : Audio.volume < 50 ? "volume-low" : "volume"
            iconColor: Audio.muted ? Qt.alpha(Theme.text, 0.5) : Theme.text
            active: bar.panel === "vol"
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton) {
                    if (Audio.sink?.audio)
                        Audio.sink.audio.muted = !Audio.sink.audio.muted;
                } else if (mouse.button === Qt.MiddleButton) {
                    if (Audio.eeRunning)
                        Audio.eeToggle();
                } else {
                    bar.toggle("vol");
                }
            }
            onWheel: wheel => {
                if (Audio.sink?.audio)
                    Audio.setVolume(Audio.sink, Audio.sink.audio.volume + (wheel.angleDelta.y > 0 ? 0.02 : -0.02));
            }
        }

        IconButton {
            // qmllint disable unresolved-type
            readonly property BluetoothAdapter adapter: Bluetooth.defaultAdapter
            readonly property bool connected: adapter?.devices.values.some(d => d.connected) ?? false

            visible: adapter !== null
            anchors.verticalCenter: parent.verticalCenter
            icon: "bluetooth"
            iconSize: 17
            iconColor: !adapter?.enabled ? Qt.alpha(Theme.text, 0.42) : connected ? Theme.bluetooth : Theme.text
            // qmllint enable unresolved-type
            active: bar.panel === "bt"
            onClicked: bar.toggle("bt")
        }

        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: {
                if (Net.wiredUp)
                    return "ethernet";
                if (Net.wifiNetwork) {
                    const s = Net.wifiNetwork.signalStrength;
                    return s >= 0.67 ? "wifi" : s >= 0.34 ? "wifi-2" : "wifi-1";
                }
                // A machine without Wi-Fi shows its unplugged cable instead.
                return Net.wifi ? "wifi-off" : "ethernet";
            }
            // Wi-Fi switched off is a choice; anything else offline is a problem.
            iconColor: Net.online ? Theme.text : Net.summary === "off" ? Theme.textFaint : Theme.warnText
            active: bar.panel === "net"
            onClicked: bar.toggle("net")
        }

        IconButton {
            id: battery

            readonly property color fill: Power.low ? Theme.warn : Power.charging ? Theme.online : Theme.text

            visible: Power.hasBattery
            anchors.verticalCenter: parent.verticalCenter
            text: Power.percent + "%"
            textColor: Power.low ? Theme.warnText : Theme.text
            active: bar.panel === "pwr"
            onClicked: bar.toggle("pwr")

            // 27x13 battery: outline, a fill as long as the charge, the nub, and a bolt over
            // it all while charging.
            Item {
                anchors.verticalCenter: parent.verticalCenter
                implicitWidth: 28
                implicitHeight: 13

                Rectangle {
                    x: 0.75
                    y: 0.75
                    width: 22.5
                    height: 11.5
                    radius: 3.5
                    color: "transparent"
                    border.width: 1.2
                    border.color: Qt.alpha(Theme.text, 0.5)
                }

                Rectangle {
                    x: 2.5
                    y: 2.5
                    width: Math.max(2, Math.round(19 * Power.percent / 100))
                    height: 8
                    radius: 2
                    color: battery.fill
                }

                Rectangle {
                    x: 24.4
                    y: 4.6
                    width: 1.6
                    height: 3.8
                    radius: 0.8
                    color: Qt.alpha(Theme.text, 0.5)
                }

                Shape {
                    visible: Power.charging
                    anchors.fill: parent
                    preferredRendererType: Shape.CurveRenderer

                    ShapePath {
                        fillColor: "#ffffff"
                        strokeColor: Theme.ink
                        strokeWidth: 0.9
                        joinStyle: ShapePath.RoundJoin
                        PathSvg {
                            path: "M13.2 1.5 8.6 7.3h3.4l-1 4.2 4.8-6.1h-3.4z"
                        }
                    }
                }
            }
        }

        Separator {
            anchors.verticalCenter: parent.verticalCenter
            // Room either side, like the other dividers.
            width: 9
            color: "transparent"

            Rectangle {
                anchors.centerIn: parent
                width: 1
                height: parent.height
                color: Theme.barDivider
            }
        }

        // Keeps the screen awake. Right click opens the power panel, the only way to it on
        // a machine without a battery.
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "coffee"
            iconSize: 17
            iconColor: Caf.enabled ? Theme.accent : Theme.text
            restColor: Caf.enabled ? Qt.alpha(Theme.accent, 0.22) : "transparent"
            active: bar.panel === "pwr" && !Power.hasBattery
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton)
                    bar.toggle("pwr");
                else
                    Caf.enabled = !Caf.enabled;
            }
        }

        DndIndicator {
            anchors.verticalCenter: parent.verticalCenter
            bar: bar
        }

        // The session menu: lock, suspend, log out, shut down, ...
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            icon: "power"
            active: bar.panel === "session"
            onClicked: bar.toggle("session")
        }
    }

    NetPanel {
        bar: bar
    }

    BtPanel {
        bar: bar
    }

    AudioPanel {
        bar: bar
    }

    PowerPanel {
        bar: bar
    }

    TsPanel {
        bar: bar
    }

    Launcher {
        bar: bar
    }

    NotifPopups {
        bar: bar
    }

    NotifCenter {
        bar: bar
    }

    Session {
        bar: bar
    }

    Osd {
        screen: bar.screen
    }

    CalendarPanel {
        bar: bar
    }
}
