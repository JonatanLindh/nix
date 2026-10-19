import qs
import QtQuick
import QtQuick.Layouts

// A rounded fill grouping related controls inside a panel; children stack vertically.
Rectangle {
    id: root

    property int padding: 12
    property alias spacing: column.spacing
    default property alias content: column.data

    Layout.fillWidth: true
    implicitWidth: column.implicitWidth + padding * 2
    implicitHeight: column.implicitHeight + padding * 2
    radius: Theme.tileRadius
    color: Theme.fill

    ColumnLayout {
        id: column
        anchors.fill: parent
        anchors.margins: root.padding
        spacing: 10
    }
}
