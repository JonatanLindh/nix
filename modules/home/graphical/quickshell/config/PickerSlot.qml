import qs
import QtQuick

// Colour-picker slot in the bar: a swatch of the last colour. Left click opens the picker
// panel, right click picks. Hidden until the picker is first used (super+shift+c), and
// again after "Remove from bar".
MouseArea {
    id: root

    required property var bar

    visible: ColorPicker.slotShown
    implicitWidth: visible ? 31 : 0
    implicitHeight: 26
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onClicked: mouse => {
        if (mouse.button === Qt.RightButton)
            ColorPicker.pick();
        else
            bar.toggle("picker");
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.buttonRadius
        color: root.bar.panel === "picker" ? Theme.fillActive : root.containsMouse ? Theme.fillHover : "transparent"
        Behavior on color {
            ColorAnimation {
                duration: Theme.fast
            }
        }
    }

    // The colour in a white ring (one bordered circle, so the ring stays even at fractional
    // scales). Nudged up half a pixel to sit on the line of the bar's icon glyphs.
    Rectangle {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: -0.5
        width: 18
        height: 18
        radius: width / 2
        color: ColorPicker.hex
        border.width: 1.5
        border.color: Qt.rgba(1, 1, 1, 0.85)
    }

    PickerPanel {
        bar: root.bar
    }
}
