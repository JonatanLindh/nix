pragma ComponentBehavior: Bound
import qs
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Services.UPower
import Quickshell.Wayland

// qmllint disable uncreatable-type
PanelWindow {
    id: bar

    // Name of the open panel ("" when none).
    property string panel: ""

    function toggle(name) {
        panel = panel === name ? "" : name;
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
        spacing: 16

        Tray {
            id: tray
            anchors.verticalCenter: parent.verticalCenter
            bar: bar
        }

        Divider {
            visible: tray.visible
        }

        Workspaces {
            screen: bar.screen
        }
    }

    Clock {
        id: clock
        anchors.centerIn: parent
    }

    // A sibling over the clock (a MouseArea inside the Row would feed back into its width).
    MouseArea {
        anchors.centerIn: clock
        width: clock.width
        height: Theme.barHeight
        cursorShape: Qt.PointingHandCursor
        onClicked: bar.toggle("cal")

        Rectangle {
            visible: bar.panel === "cal"
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 5
            width: parent.width
            height: 1
            color: Theme.accent
        }
    }

    // Hangs off the clock's right so the clock never moves.
    Media {
        anchors.left: clock.right
        anchors.leftMargin: Theme.gap
        anchors.verticalCenter: parent.verticalCenter
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.barPadding
        height: parent.height
        spacing: Theme.gap

        // Only while something records the mic, camera or screen.
        PrivacyIndicators {}

        Divider {
            visible: Privacy.mic || Privacy.camera || Privacy.screen
        }

        DndIndicator {
            bar: bar
        }

        Field {
            value: "caf"
            valueColor: Caf.enabled ? Theme.accent : Theme.off
            open: bar.panel === "pwr" && !Power.hasBattery && !Power.profilesAvailable
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton)
                    bar.toggle("pwr");
                else
                    Caf.enabled = !Caf.enabled;
            }
        }

        Field {
            visible: Power.profilesAvailable
            label: "pwr"
            cols: 4
            alignRight: false
            value: Power.shortName(Power.profile)
            valueColor: Power.profile === PowerProfile.Balanced ? Theme.text : Theme.accent
            onClicked: mouse => {
                if (mouse.button === Qt.RightButton)
                    bar.toggle("pwr");
                else
                    Power.cycle();
            }
        }

        Divider {}

        Field {
            visible: Brightness.available
            label: "bri"
            cols: 3
            value: `${Brightness.percent}`
            cursorShape: Qt.SizeVerCursor
            onWheel: wheel => Brightness.step(wheel.angleDelta.y > 0)
        }

        Field {
            label: "vol"
            cols: 3
            value: Audio.muted ? "off" : `${Audio.volume}`
            valueColor: Audio.muted ? Theme.off : Theme.text
            open: bar.panel === "vol"
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

            BarText {
                visible: Audio.eeRunning
                text: " fx"
                color: Audio.fx ? Theme.accent : Theme.off
            }
        }

        Field {
            // qmllint disable unresolved-type
            readonly property BluetoothAdapter adapter: Bluetooth.defaultAdapter

            visible: adapter !== null
            label: "bt"
            cols: 3
            value: adapter?.enabled ? `${adapter.devices.values.filter(d => d.connected).length}` : "off"
            // qmllint enable unresolved-type
            valueColor: adapter?.enabled ? Theme.text : Theme.off
            open: bar.panel === "bt"
            onClicked: bar.toggle("bt")
        }

        Field {
            visible: Ts.available
            label: "ts"
            value: Ts.backend === "NeedsLogin" ? "login" : !Ts.up ? "off" : Ts.exitNode ? Ts.exitNode.name : "on"
            valueColor: Ts.backend === "NeedsLogin" ? Theme.warn : !Ts.up ? Theme.off : Ts.exitNode ? Theme.accent : Theme.text
            open: bar.panel === "ts"
            onClicked: bar.toggle("ts")

            // Something is reachable from the public internet.
            BarText {
                visible: Ts.funnelActive
                text: " fn"
                color: Theme.accent
            }
        }

        Field {
            label: "net"
            value: Net.summary
            valueColor: Net.online ? Theme.text : Net.summary === "off" ? Theme.off : Theme.warn
            open: bar.panel === "net"
            onClicked: bar.toggle("net")
        }

        Field {
            visible: Power.hasBattery
            label: "bat"
            cols: 4
            value: Power.percent + (Power.charging ? "+" : "")
            valueColor: Power.low ? Theme.warn : Power.charging ? Theme.accent : Theme.text
            bold: Power.low
            open: bar.panel === "pwr"
            onClicked: bar.toggle("pwr")
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

    Clip {
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

    Passwords {
        bar: bar
    }

    Osd {
        screen: bar.screen
    }

    CalendarPanel {
        bar: bar
    }
}
