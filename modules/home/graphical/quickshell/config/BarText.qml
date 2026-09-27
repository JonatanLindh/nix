import qs
import QtQuick

Text {
    color: Theme.text
    font.family: Theme.font
    font.pixelSize: Theme.fontSize
    verticalAlignment: Text.AlignVCenter
    // Grayscale antialiasing; subpixel (LCD) AA gives colour fringes on the translucent bar.
    renderType: Text.CurveRendering
}
