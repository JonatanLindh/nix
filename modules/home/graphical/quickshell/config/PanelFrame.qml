import qs
import QtQuick
import QtQuick.Layouts

// The glass card every panel sits in: header (optional back arrow, title, actions on the
// right), the body, and an optional footer under a hairline.
Glass {
    id: root

    property string title: ""
    property bool showBack: false
    property bool showHeader: title !== "" || showBack
    default property alias content: body.data
    property alias actions: actionsRow.data
    property alias footer: footerRow.data
    property alias spacing: body.spacing

    signal dismissed
    signal back

    implicitHeight: column.implicitHeight + Theme.panelPadding * 2
    focus: true
    Keys.onEscapePressed: dismissed()

    ColumnLayout {
        id: column

        anchors.fill: parent
        anchors.margins: Theme.panelPadding
        spacing: 10

        RowLayout {
            visible: root.showHeader
            Layout.fillWidth: true
            Layout.leftMargin: root.showBack ? 0 : 6
            Layout.rightMargin: 2
            spacing: 8

            CircleButton {
                visible: root.showBack
                icon: "chevron-left"
                iconSize: 16
                onClicked: root.back()
            }
            UiText {
                Layout.fillWidth: true
                text: root.title
                font.pixelSize: Theme.title
                font.weight: Font.DemiBold
            }
            Row {
                id: actionsRow
                spacing: 8
                Layout.alignment: Qt.AlignVCenter
            }
        }

        ColumnLayout {
            id: body
            Layout.fillWidth: true
            spacing: 10
        }

        Rectangle {
            visible: footerRow.children.length > 0
            Layout.fillWidth: true
            Layout.leftMargin: 4
            Layout.rightMargin: 4
            implicitHeight: 1
            color: Theme.divider
        }

        RowLayout {
            id: footerRow
            visible: children.length > 0
            Layout.fillWidth: true
            spacing: 8
        }
    }
}
