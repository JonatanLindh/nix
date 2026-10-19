import qs
import QtQuick
import QtQuick.Layouts

// Circular quick toggle with a caption under it (control centre). Accent when on.
ColumnLayout {
    id: root

    property string icon: ""
    property string text: ""
    property bool checked: false
    property int size: 48

    signal toggled

    spacing: 6

    MouseArea {
        Layout.alignment: Qt.AlignHCenter
        implicitWidth: root.size
        implicitHeight: root.size
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled()

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: root.checked ? Theme.accent : parent.containsMouse ? Theme.fillActive : Theme.fillHover
            Behavior on color {
                ColorAnimation {
                    duration: Theme.fast
                }
            }
        }
        Icon {
            anchors.centerIn: parent
            name: root.icon
            size: Math.round(root.size * 0.42)
            strokeWidth: 2
            color: root.checked ? Theme.accentInk : Theme.text
        }
    }

    UiText {
        visible: root.text !== ""
        Layout.alignment: Qt.AlignHCenter
        Layout.maximumWidth: root.size + 30
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        elide: Text.ElideNone
        text: root.text
        color: Theme.textDim
        font.pixelSize: Theme.tiny
        lineHeight: 1.1
    }
}
