pragma ComponentBehavior: Bound
import qs
import QtQuick

// "[ctrl][↵] Copy path" — key caps followed by what they do.
Row {
    id: root

    property var keys: []
    property string text: ""

    spacing: 5

    Repeater {
        model: root.keys
        Kbd {
            required property string modelData
            // Delegates start without a parent, hence the optional chain.
            anchors.verticalCenter: parent?.verticalCenter
            text: modelData
        }
    }
    UiText {
        anchors.verticalCenter: parent.verticalCenter
        text: root.text
        dim: true
        font.pixelSize: Theme.small
    }
}
