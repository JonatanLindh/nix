import qs
import QtQuick

// Thin vertical line between groups in the bar.
Rectangle {
    anchors.verticalCenter: parent?.verticalCenter
    implicitWidth: 1
    implicitHeight: 14
    color: Theme.barDivider
}
