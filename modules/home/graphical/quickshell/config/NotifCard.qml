pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Widgets

// What a notification says, shared by the popups and the history: the app's icon (or a
// coloured letter), app name and time, the summary, the body and the picture if it has one.
// `entry` is a Notifs entry; `time` is what goes top right ("now", "14:02").
RowLayout {
    id: root

    required property var entry
    property string time: ""
    property int bodyLines: 2
    // Room kept free at the end of the first line for the × that appears on hover.
    property int headerInset: 0

    readonly property string iconSource: source(entry.icon)
    // Small, squarish pictures are avatars and take the icon's place; anything else is
    // shown full width under the text.
    readonly property bool imageIsIcon: picture.status === Image.Ready && picture.implicitWidth <= 256 && picture.implicitWidth / Math.max(1, picture.implicitHeight) < 1.3
    readonly property bool hasThumb: picture.status === Image.Ready && !imageIsIcon

    // A theme icon name, a path or a url, as something Image can load ("" if none).
    function source(icon) {
        if (!icon)
            return "";
        // An image-path hint arrives as an icon-provider url, which draws a checkerboard
        // rather than failing when the file or icon doesn't exist.
        if (icon.startsWith("image://icon/"))
            icon = icon.slice(13);
        if (icon.startsWith("/"))
            return "file://" + icon;
        if (icon.includes("://"))
            return icon;
        return Quickshell.iconPath(icon, true);
    }

    // Stable per-app colour for the letter tile.
    function tint(name) {
        const colors = ["#8a3f78", "#2f8f7f", "#5865c9", "#2b4a6e", "#9a5b2e", "#3f7a4a", "#7a4fa8", "#a8434f"];
        let h = 0;
        for (const c of name)
            h = (h * 31 + c.charCodeAt(0)) >>> 0;
        return colors[h % colors.length];
    }

    spacing: 12

    // Loaded off screen to learn its size; drawn by one of the two places below.
    Image {
        id: picture
        visible: false
        source: root.source(root.entry.image)
        asynchronous: true
    }

    Item {
        Layout.alignment: Qt.AlignTop
        implicitWidth: 32
        implicitHeight: 32

        ClippingRectangle {
            anchors.fill: parent
            radius: 9
            color: root.imageIsIcon || appIcon.status === Image.Ready ? "transparent" : root.tint(root.entry.app)

            UiText {
                visible: !root.imageIsIcon && appIcon.status !== Image.Ready
                anchors.centerIn: parent
                text: root.entry.app.charAt(0).toUpperCase()
                color: "white"
                font.pixelSize: 14
                font.weight: Font.DemiBold
            }
            Image {
                visible: root.imageIsIcon
                anchors.fill: parent
                source: root.imageIsIcon ? picture.source : ""
                fillMode: Image.PreserveAspectCrop
                sourceSize.width: 64
                sourceSize.height: 64
                smooth: true
            }
        }
        IconImage {
            id: appIcon
            visible: !root.imageIsIcon && status === Image.Ready
            anchors.fill: parent
            source: root.iconSource
            asynchronous: true
        }
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: 3

        RowLayout {
            Layout.fillWidth: true
            Layout.rightMargin: root.headerInset
            spacing: 8

            UiText {
                Layout.fillWidth: true
                text: root.entry.app
                color: root.entry.critical ? Theme.warnText : Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.66)
                font.pixelSize: Theme.small
            }
            UiText {
                visible: root.time !== ""
                text: root.time
                mono: true
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.66)
                font.pixelSize: Theme.tiny
            }
        }

        UiText {
            Layout.fillWidth: true
            visible: text !== ""
            text: root.entry.summary.replace(/\s+/g, " ")
            font.weight: Font.DemiBold
        }

        UiText {
            Layout.fillWidth: true
            visible: text !== ""
            text: root.entry.body
            color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.82)
            font.pixelSize: Theme.small
            wrapMode: Text.Wrap
            maximumLineCount: root.bodyLines
            lineHeight: 1.15
        }

        ClippingRectangle {
            visible: root.hasThumb
            Layout.fillWidth: true
            Layout.topMargin: 6
            implicitHeight: 96
            radius: 10
            color: "transparent"

            Image {
                anchors.fill: parent
                source: root.hasThumb ? picture.source : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
            }
        }
    }
}
