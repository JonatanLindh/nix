pragma Singleton
import QtQuick
import Quickshell

// Glass: translucent surfaces over the blurred desktop, Geist, white highlights.
// Design: "Quickshell Bar Concepts" canvas, section D.
Singleton {
    id: root

    // "On"/selected highlight and what's drawn on it. Deliberately not a colour: a teal accent
    // only showed up here and there and looked out of place, so it's the same near-white.
    readonly property color accent: white
    readonly property color accentInk: ink

    // Text, from strongest to faintest.
    readonly property color text: "#eef4f3"
    readonly property color textDim: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.68)
    readonly property color textFaint: Qt.rgba(235 / 255, 243 / 255, 242 / 255, 0.45)

    // Surfaces. The bar is lighter glass than the panels floating over windows.
    readonly property color barBg: Qt.rgba(8 / 255, 18 / 255, 22 / 255, 0.46)
    readonly property color glass: Qt.rgba(14 / 255, 26 / 255, 30 / 255, 0.62)
    readonly property color glassBorder: Qt.rgba(1, 1, 1, 0.14)
    // Fills on glass: resting tiles, hover, pressed/open.
    readonly property color fill: Qt.rgba(1, 1, 1, 0.08)
    readonly property color fillHover: Qt.rgba(1, 1, 1, 0.12)
    readonly property color fillActive: Qt.rgba(1, 1, 1, 0.18)
    readonly property color divider: Qt.rgba(1, 1, 1, 0.08)
    readonly property color barDivider: Qt.rgba(1, 1, 1, 0.16)
    // Off switches / empty tracks.
    readonly property color track: Qt.rgba(1, 1, 1, 0.14)

    // Solid near-white (slider fills, today, selected tiles) and the ink drawn on it.
    readonly property color white: "#f4f7f6"
    readonly property color ink: "#0b1d20"

    // Status colours.
    readonly property color warn: "#ff6b6b"
    readonly property color warnText: "#ff8a8a"
    readonly property color camera: "#3ddc78"
    readonly property color mic: "#ffa94d"
    readonly property color screen: "#b79cff"
    readonly property color online: "#3ddc78"

    // Type.
    readonly property string font: "Geist"
    readonly property string mono: "Geist Mono"
    readonly property int fontSize: 13
    readonly property int small: 12
    readonly property int tiny: 11
    readonly property int title: 15

    // Geometry.
    readonly property int barHeight: 34
    readonly property int barPadding: 8
    readonly property int panelWidth: 360
    readonly property int panelPadding: 14
    readonly property int panelGap: 8
    readonly property int panelRadius: 22
    readonly property int tileRadius: 16
    readonly property int rowRadius: 10
    readonly property int buttonRadius: 8
    readonly property int gap: 10

    // Motion.
    readonly property int fast: 120
    readonly property int medium: 180
}
