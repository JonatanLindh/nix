pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.UPower

Panel {
    id: root

    name: "pwr"
    title: Power.hasBattery ? "Battery" : "Power"
    spacing: 12

    actions: [
        UiText {
            visible: Power.hasBattery
            anchors.verticalCenter: parent?.verticalCenter
            text: Power.charging ? "Charging" : Power.onAc ? "Plugged in" : "On battery"
            dim: true
            font.pixelSize: Theme.small
        },
        // The full session menu (super+m).
        CircleButton {
            icon: "power"
            iconSize: 16
            onClicked: root.bar.toggle("session")
        }
    ]

    Tile {
        visible: Power.hasBattery
        padding: 16

        RowLayout {
            Layout.fillWidth: true
            spacing: 16

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                UiText {
                    text: Power.percent + "%"
                    color: Power.low ? Theme.warnText : Theme.text
                    font.pixelSize: 34
                    font.weight: Font.DemiBold
                    font.letterSpacing: -0.7
                    font.features: {
                        "tnum": 1
                    }
                }
                UiText {
                    visible: text !== ""
                    text: Power.timeLeft || (Power.onAc ? "Fully charged" : "")
                    dim: true
                    font.pixelSize: Theme.small
                }
            }

            // Large battery glyph, filled to the charge.
            Item {
                implicitWidth: 74
                implicitHeight: 34

                Rectangle {
                    width: 66
                    height: 34
                    radius: 10
                    color: "transparent"
                    border.width: 2
                    border.color: Qt.alpha(Theme.text, 0.5)

                    Rectangle {
                        x: 5
                        y: 5
                        width: Math.max(6, Math.round(56 * Power.percent / 100))
                        height: 24
                        radius: 5.5
                        color: Power.low ? Theme.warn : Theme.text
                    }
                    Icon {
                        visible: Power.charging
                        anchors.centerIn: parent
                        name: "bolt"
                        size: 18
                        strokeWidth: 2.4
                        color: Power.percent > 55 ? Theme.ink : Theme.text
                    }
                }
                Rectangle {
                    x: 69
                    y: 12
                    width: 3
                    height: 10
                    radius: 1.5
                    color: Qt.alpha(Theme.text, 0.5)
                }
            }
        }
    }

    // Screen brightness (laptop); it used to sit in the control centre.
    Tile {
        visible: Brightness.available
        spacing: 8

        UiText {
            text: "Display"
            dim: true
            font.pixelSize: Theme.small
        }
        SplitSlider {
            icon: "sun"
            value: Brightness.current / Brightness.max
            onMoved: v => {
                // Never fully dark; shown right away, the sysfs watch catches up.
                const pct = Math.max(1, Math.round(v * 100));
                Brightness.current = Math.round(pct / 100 * Brightness.max);
                Quickshell.execDetached(["brightnessctl", "set", pct + "%"]);
            }
        }
    }

    ColumnLayout {
        visible: Power.profilesAvailable
        Layout.fillWidth: true
        spacing: 8

        UiText {
            Layout.leftMargin: 4
            text: "Power mode"
            dim: true
            font.pixelSize: Theme.small
            font.weight: Font.Medium
        }

        RowLayout {
            id: modes
            Layout.fillWidth: true
            spacing: 8

            Repeater {
                model: Power.profiles

                MouseArea {
                    id: mode

                    required property int modelData
                    readonly property bool on: Power.profile === modelData

                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    implicitHeight: 78
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Power.setProfile(modelData)

                    Rectangle {
                        anchors.fill: parent
                        radius: 14
                        color: mode.on ? Theme.accent : mode.containsMouse ? Theme.fillHover : Theme.fill
                        Behavior on color {
                            ColorAnimation {
                                duration: Theme.fast
                            }
                        }
                    }
                    Column {
                        anchors.centerIn: parent
                        spacing: 8

                        Icon {
                            anchors.horizontalCenter: parent.horizontalCenter
                            name: mode.modelData === PowerProfile.Performance ? "bolt" : mode.modelData === PowerProfile.PowerSaver ? "leaf" : "gauge"
                            size: 20
                            strokeWidth: 2
                            color: mode.on ? Theme.accentInk : Theme.text
                        }
                        UiText {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: mode.modelData === PowerProfile.Performance ? "Performance" : mode.modelData === PowerProfile.PowerSaver ? "Saver" : "Balanced"
                            color: mode.on ? Theme.accentInk : Theme.text
                            font.pixelSize: Theme.small
                            font.weight: Font.Medium
                        }
                    }
                }
            }
        }
    }

    Tile {
        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            Rectangle {
                implicitWidth: 34
                implicitHeight: 34
                radius: 17
                color: Caf.enabled ? Theme.accent : Theme.fillHover

                Icon {
                    anchors.centerIn: parent
                    name: "coffee"
                    size: 17
                    strokeWidth: 2.1
                    color: Caf.enabled ? Theme.accentInk : Theme.text
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                UiText {
                    Layout.fillWidth: true
                    text: "Caffeine"
                    font.weight: Font.DemiBold
                }
                UiText {
                    Layout.fillWidth: true
                    text: "Keep the screen on, skip auto-lock"
                    dim: true
                    font.pixelSize: Theme.small
                }
            }
            Switch {
                checked: Caf.enabled
                onToggled: Caf.enabled = !Caf.enabled
            }
        }
    }
}
