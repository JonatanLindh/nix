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
        spacing: 10

        HoverHandler {
            id: hover
        }

        Repeater {
            model: Notifs.shown

            delegate: Glass {
                id: card

                required property var modelData

                Layout.fillWidth: true
                implicitHeight: content.implicitHeight + 24
                radius: 18
                border.color: modelData.critical ? Qt.alpha(Theme.warn, 0.6) : Theme.glassBorder

                // Click: default action (if any), then out of the way.
                MouseArea {
                    id: area

                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: card.modelData.hasDefault ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: Notifs.activate(card.modelData.id)
                }

                ColumnLayout {
                    id: content

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12
                    spacing: 10

                    NotifCard {
                        Layout.fillWidth: true
                        entry: card.modelData
                        time: "now"
                        bodyLines: 4
                        headerInset: closeButton.visible ? 22 : 0
                    }

                    GridLayout {
                        visible: card.modelData.actions.length > 0
                        Layout.fillWidth: true
                        columns: 2
                        rowSpacing: 8
                        columnSpacing: 8

                        Repeater {
                            model: card.modelData.actions

                            delegate: PillButton {
                                required property var modelData

                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                pillHeight: 30
                                text: modelData.text
                                onClicked: Notifs.invoke(card.modelData.id, modelData.id)
                            }
                        }
                    }
                }

                // Over the card's own MouseArea, which would otherwise take the hover.
                CircleButton {
                    id: closeButton

                    visible: hover.hovered && cardHover.hovered
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.margins: 8
                    size: 22
                    icon: "close"
                    iconSize: 12
                    restColor: Theme.fillHover
                    onClicked: Notifs.hidePopup(card.modelData.id)
                }

                HoverHandler {
                    id: cardHover
                }
            }
        }
    }
}
