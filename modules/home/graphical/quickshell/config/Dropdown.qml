pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts

// Single-choice field: always shows the current choice; clicking unfolds the options right
// below it (pushing content down, so it never spills outside the panel window).
// model: [{ text, icon?, note?, disabled? }]. Emits activated(index); the owner sets
// currentIndex from its own state.
ColumnLayout {
    id: root

    property var model: []
    property int currentIndex: -1
    // Dim prefix inside the field, e.g. "Preset".
    property string label: ""
    // Extra dim text after the value, e.g. "Routing all traffic".
    property string sublabel: ""
    property bool enabled: true
    property bool open: false

    signal activated(int index)

    readonly property var current: currentIndex >= 0 && currentIndex < model.length ? model[currentIndex] : null

    Layout.fillWidth: true
    spacing: 4
    onEnabledChanged: if (!enabled)
        open = false

    MouseArea {
        Layout.fillWidth: true
        implicitHeight: 38
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        opacity: root.enabled ? 1 : 0.5
        onClicked: root.open = !root.open

        Rectangle {
            anchors.fill: parent
            radius: 12
            color: parent.containsMouse || root.open ? Theme.fillHover : Theme.fill
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 10

            UiText {
                visible: root.label !== ""
                text: root.label
                dim: true
                font.pixelSize: Theme.small
            }
            Icon {
                visible: (root.current?.icon ?? "") !== ""
                name: root.current?.icon ?? ""
                size: 15
                color: Theme.textDim
            }
            UiText {
                Layout.fillWidth: true
                text: root.current?.text ?? ""
            }
            UiText {
                visible: root.sublabel !== ""
                text: root.sublabel
                dim: true
                font.pixelSize: Theme.small
            }
            Icon {
                name: "chevron-down"
                size: 14
                strokeWidth: 2.2
                color: Theme.textDim
                rotation: root.open ? 180 : 0
                Behavior on rotation {
                    NumberAnimation {
                        duration: Theme.fast
                    }
                }
            }
        }
    }

    Rectangle {
        visible: root.open
        Layout.fillWidth: true
        implicitHeight: options.implicitHeight + 8
        radius: 14
        color: Qt.rgba(1, 1, 1, 0.06)
        border.width: 1
        border.color: Theme.glassBorder

        ColumnLayout {
            id: options
            anchors.fill: parent
            anchors.margins: 4
            spacing: 2

            Repeater {
                model: root.model

                MouseArea {
                    id: option

                    required property var modelData
                    required property int index
                    readonly property bool disabled: modelData.disabled ?? false

                    Layout.fillWidth: true
                    implicitHeight: 34
                    enabled: !disabled
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    // Stays open after a pick, so neighbouring options can be tried in turn.
                    onClicked: root.activated(index)

                    Rectangle {
                        anchors.fill: parent
                        radius: 9
                        color: option.containsMouse ? Theme.fillHover : "transparent"
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 10
                        anchors.rightMargin: 10
                        spacing: 10
                        opacity: option.disabled ? 0.45 : 1

                        Icon {
                            name: "check"
                            size: 16
                            strokeWidth: 2.6
                            color: Theme.accent
                            opacity: option.index === root.currentIndex ? 1 : 0
                        }
                        Icon {
                            visible: (option.modelData.icon ?? "") !== ""
                            name: option.modelData.icon ?? ""
                            size: 15
                            color: Theme.textDim
                        }
                        UiText {
                            Layout.fillWidth: true
                            text: option.modelData.text ?? ""
                        }
                        UiText {
                            visible: (option.modelData.note ?? "") !== ""
                            text: option.modelData.note ?? ""
                            faint: true
                            font.pixelSize: Theme.tiny
                        }
                    }
                }
            }
        }
    }
}
