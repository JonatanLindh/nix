import qs
import QtQuick

// Text in the Glass type (Geist). `dim`/`faint` for secondary text, `mono` for numbers,
// addresses and codes.
Text {
    property bool dim: false
    property bool faint: false
    property bool mono: false
    // Equal-width digits in the proportional face, for numbers that change in place.
    property bool tabular: false

    color: faint ? Theme.textFaint : dim ? Theme.textDim : Theme.text
    font.family: mono ? Theme.mono : Theme.font
    font.pixelSize: Theme.fontSize
    // Regular Geist reads thin on glass with grayscale antialiasing; Medium matches the design.
    font.weight: Font.Medium
    font.features: mono || tabular ? {
        "tnum": 1
    } : {}
    verticalAlignment: Text.AlignVCenter
    elide: Text.ElideRight
    // FreeType with fontconfig's hinting (slight, grayscale here) keeps small text on the pixel
    // grid; curve rendering left it soft and uneven at these sizes.
    renderType: Text.NativeRendering
}
