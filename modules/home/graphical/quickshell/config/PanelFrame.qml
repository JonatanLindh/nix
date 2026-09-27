import qs
import QtQuick
import QtQuick.Layouts

// The look shared by all panels: title + actions, a hairline, the body, an optional footer.
Rectangle {
    id: root

    property string title: ""
    default property alias content: body.data
    property alias actions: actionsRow.data
    property alias footer: footerRow.data

    signal dismissed

    implicitHeight: column.implicitHeight
    color: Theme.barBg
    focus: true
    Keys.onEscapePressed: dismissed()

    ColumnLayout {
        id: column

        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Theme.panelPadding
            Layout.topMargin: 14
            Layout.bottomMargin: 14

            BarText {
                text: root.title
                color: Theme.bright
                font.bold: true
            }

            Item {
                Layout.fillWidth: true
            }

            Row {
                id: actionsRow
                spacing: 14
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: Theme.line
        }

        ColumnLayout {
            id: body

            Layout.fillWidth: true
            Layout.bottomMargin: 18
            spacing: 0
        }

        Rectangle {
            visible: footerRow.children.length > 0
            Layout.fillWidth: true
            implicitHeight: 1
            color: Theme.line
        }

        RowLayout {
            id: footerRow

            visible: children.length > 0
            Layout.fillWidth: true
            Layout.margins: Theme.panelPadding
            Layout.topMargin: 12
            Layout.bottomMargin: 12
            spacing: 18
        }
    }
}
