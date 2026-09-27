import qs
import QtQuick
import Quickshell.Services.Mpris

// Dim "▶ artist - title" next to the clock; only while something has a track.
MouseArea {
    id: root

    // playerctld proxies other players; skip it to avoid duplicates.
    readonly property var players: Mpris.players.values.filter(p => !p.dbusName.includes("playerctld") && p.trackTitle)
    // Prefer whatever is playing, else the first player.
    readonly property MprisPlayer player: players.find(p => p.isPlaying) ?? players[0] ?? null

    visible: player !== null
    implicitWidth: label.implicitWidth
    implicitHeight: Theme.barHeight
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    cursorShape: Qt.PointingHandCursor
    onClicked: mouse => {
        if (mouse.button === Qt.RightButton)
            player.next();
        else if (mouse.button === Qt.MiddleButton)
            player.previous();
        else
            player.togglePlaying();
    }

    Row {
        id: label

        anchors.verticalCenter: parent.verticalCenter
        spacing: 0

        // Own fixed box, so the play/pause glyph swap can't move the title.
        BarText {
            width: icon.advanceWidth * 2
            height: Theme.barHeight
            text: root.player?.isPlaying ? "▶" : "‖"
            color: Theme.label

            TextMetrics {
                id: icon
                font.family: Theme.font
                font.pixelSize: Theme.fontSize
                text: "0"
            }
        }

        BarText {
            readonly property string title: {
                const p = root.player;
                if (!p)
                    return "";
                const s = [p.trackArtist, p.trackTitle].filter(x => x).join(" - ").toLowerCase();
                return s.length > 50 ? s.slice(0, 49) + "…" : s;
            }

            height: Theme.barHeight
            text: title
            color: Theme.label
        }
    }
}
