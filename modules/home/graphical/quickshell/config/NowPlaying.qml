pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Layouts
import Quickshell.Services.Mpris
import Quickshell.Widgets

// Now playing: cover art, title, artist and transport buttons. Hidden while no player has
// a track.
Tile {
    id: root

    // playerctld proxies other players; skip it to avoid duplicates.
    readonly property var players: Mpris.players.values.filter(p => !p.dbusName.includes("playerctld") && p.trackTitle)
    // Prefer whatever is playing, else the first player.
    readonly property MprisPlayer player: players.find(p => p.isPlaying) ?? players[0] ?? null

    visible: player !== null
    padding: 10

    RowLayout {
        Layout.fillWidth: true
        spacing: 12

        ClippingRectangle {
            implicitWidth: 46
            implicitHeight: 46
            radius: 10
            color: Qt.rgba(1, 1, 1, 0.1)

            Icon {
                visible: art.status !== Image.Ready
                anchors.centerIn: parent
                name: "music"
                size: 20
                color: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.6)
            }
            Image {
                id: art
                anchors.fill: parent
                source: root.player?.trackArtUrl ?? ""
                sourceSize.width: 92
                sourceSize.height: 92
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2

            UiText {
                Layout.fillWidth: true
                text: root.player?.trackTitle ?? ""
                font.weight: Font.DemiBold
            }
            UiText {
                visible: text !== ""
                Layout.fillWidth: true
                text: root.player?.trackArtist ?? ""
                dim: true
                font.pixelSize: Theme.small
            }
        }

        Row {
            CircleButton {
                size: 32
                iconSize: 16
                icon: "prev"
                iconColor: Theme.text
                opacity: root.player?.canGoPrevious ? 1 : 0.4
                onClicked: root.player?.previous()
            }
            CircleButton {
                size: 32
                iconSize: 18
                icon: root.player?.isPlaying ? "pause" : "play"
                iconColor: Theme.text
                onClicked: root.player?.togglePlaying()
            }
            CircleButton {
                size: 32
                iconSize: 16
                icon: "next"
                iconColor: Theme.text
                opacity: root.player?.canGoNext ? 1 : 0.4
                onClicked: root.player?.next()
            }
        }
    }
}
