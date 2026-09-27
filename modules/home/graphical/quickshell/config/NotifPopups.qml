pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Hyprland
import Quickshell.Wayland

// Notification popups under the right end of the bar, newest on top, on the focused
// monitor only. Hovering them (or having a panel open over them) holds their timeouts.
// qmllint disable uncreatable-type
PanelWindow {
    id: root

    required property var bar
    readonly property bool focused: Hyprland.focusedMonitor?.name === bar.screen?.name
    // Panels open in the same corner; the popups wait until they're closed.
    readonly property bool covered: bar.panel !== ""

    screen: bar.screen
    visible: focused && !covered && Notifs.shown.length > 0
    anchors.top: true
    anchors.right: true
    // qmllint disable unqualified unresolved-type
    margins.top: Theme.barHeight + 4
    margins.right: 8
    // qmllint enable unqualified unresolved-type
    exclusionMode: ExclusionMode.Ignore
    implicitWidth: Theme.panelWidth
    // Fixed height (room for 4 popups) so popups coming and going don't resize the surface,
    // which Hyprland animates; the mask below keeps the empty part click-through.
    implicitHeight: 720
    color: "transparent"
    // Only the popups take clicks, not the gaps between them.
    mask: Region {
        item: column
    }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.namespace: "quickshell:notifs"
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    // Only the focused monitor's popups say whether expiry is held.
    Binding {
        target: Notifs
        property: "paused"
        value: hover.hovered || root.covered
        when: root.focused
        restoreMode: Binding.RestoreNone
    }

    ColumnLayout {
        id: column

        width: parent.width
        spacing: 6

        HoverHandler {
            id: hover
        }

        Repeater {
            model: Notifs.shown

            delegate: Rectangle {
                id: card

                required property var modelData

                Layout.fillWidth: true
                implicitHeight: content.implicitHeight + 24
                color: area.containsMouse ? Qt.alpha(Theme.hover, 0.95) : Theme.barBg

                // Click: default action (if any), then out of the way.
                MouseArea {
                    id: area

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: card.modelData.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Notifs.activate(card.modelData.id)
                }

                Rectangle {
                    visible: card.modelData.critical
                    width: 2
                    height: parent.height
                    color: Theme.warn
                }

                ColumnLayout {
                    id: content

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.leftMargin: Theme.panelPadding
                    anchors.rightMargin: Theme.panelPadding
                    anchors.topMargin: 12
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        BarText {
                            Layout.fillWidth: true
                            text: card.modelData.app
                            elide: Text.ElideRight
                            color: card.modelData.critical ? Theme.warn : Theme.label
                        }

                        Action {
                            text: "×"
                            onClicked: Notifs.hidePopup(card.modelData.id)
                        }
                    }

                    BarText {
                        Layout.fillWidth: true
                        visible: text !== ""
                        text: card.modelData.summary
                        color: Theme.bright
                        font.bold: true
                        wrapMode: Text.Wrap
                        maximumLineCount: 2
                        elide: Text.ElideRight
                    }

                    BarText {
                        Layout.fillWidth: true
                        visible: text !== ""
                        text: card.modelData.body
                        wrapMode: Text.Wrap
                        maximumLineCount: 4
                        elide: Text.ElideRight
                        lineHeight: 1.2
                    }

                    Row {
                        visible: card.modelData.actions.length > 0
                        Layout.topMargin: 6
                        spacing: 14

                        Repeater {
                            model: card.modelData.actions

                            delegate: Action {
                                required property var modelData

                                text: modelData.text
                                onClicked: Notifs.invoke(card.modelData.id, modelData.id)
                            }
                        }
                    }
                }
            }
        }
    }
}
