pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Shapes

// A 24x24 icon from Icons.qml, drawn as vector paths so it stays crisp and takes any colour.
Item {
    id: root

    property string name: ""
    property color color: Theme.text
    property int size: 18
    // In 24-unit icon space, like the SVG stroke-width on the canvas.
    property real strokeWidth: 1.9

    readonly property var paths: Icons.paths[name] ?? ({})

    implicitWidth: size
    implicitHeight: size

    Shape {
        width: 24
        height: 24
        scale: root.size / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            strokeColor: "transparent"
            fillColor: Qt.alpha(root.color, 0.45)
            PathSvg {
                path: root.paths.fillFaint ?? ""
            }
        }
        ShapePath {
            strokeColor: Qt.alpha(root.color, 0.3)
            strokeWidth: root.strokeWidth
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: root.paths.strokeFaint ?? ""
            }
        }
        ShapePath {
            strokeColor: "transparent"
            fillColor: root.color
            PathSvg {
                path: root.paths.fill ?? ""
            }
        }
        ShapePath {
            strokeColor: root.color
            strokeWidth: root.strokeWidth
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: root.paths.stroke ?? ""
            }
        }
    }
}
