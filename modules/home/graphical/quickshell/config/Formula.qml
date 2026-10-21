pragma ComponentBehavior: Bound
import qs
import QtQuick
import QtQuick.Shapes

// A formula, set the way TeX would: fractions stacked, roots under a radical, powers raised,
// everything lined up on the math axis (the height of a fraction bar), in New Computer Modern
// Math (Typst's math font). `node` is the tree qs-find makes from qalc's LaTeX
// (helpers/src/bin/qs-find/tex.rs):
//   "text" | [a, b, ...] | { v: "x" } | { frac: [a, b] } | { sqrt: a, n?: b } | { sup: [a, b] }
//   | { sub: [a, b] } | { delim: ["(", a, ")"] } | { mat: [[a, ...], ...], l: "[", r: "]" }
// Pieces nest by loading this file again. Each one tells its parent how far down from its
// top the axis is (`axis`), so a row can line them up.
// Not called Math: that name would hide JavaScript's Math in every file that imports qs.
Item {
    id: root

    property var node: ""
    property real size: 32
    property color color: Theme.white
    // How many fractions or scripts deep this is; parts of nested fractions get smaller.
    property int level: 0

    readonly property string family: "NewComputerModern Math"
    readonly property string kind: {
        const n = node;
        if (typeof n === "string" || !n)
            return "text";
        // Lists come through property bindings as Qt sequences, not JS arrays.
        if (Array.isArray(n) || typeof n.length === "number")
            return "row";
        if (n.v !== undefined)
            return "var";
        if (n.frac)
            return "frac";
        if (n.sqrt !== undefined)
            return "sqrt";
        if (n.sup)
            return "sup";
        if (n.sub)
            return "sub";
        if (n.delim)
            return "delim";
        if (n.mat)
            return "mat";
        return "text";
    }
    readonly property real axis: body.item ? body.item.axis : 0

    // Line thickness: fraction bars, the radical, drawn brackets.
    readonly property real rule: Math.max(1, size * 0.045)
    // From the top of a line of text down to the axis: half an x-height above the baseline.
    readonly property real textAxis: metrics.ascent - metrics.xHeight / 2
    readonly property real lineHeight: metrics.ascent + metrics.descent

    implicitWidth: body.item ? body.item.implicitWidth : 0
    implicitHeight: body.item ? body.item.implicitHeight : 0

    // Letters as the Unicode math italics (𝑥, 𝑛), which is how math fonts carry them.
    function italic(s) {
        let out = "";
        for (const ch of s) {
            const c = ch.codePointAt(0);
            if (ch === "h")
                out += "ℎ";
            else if (c >= 97 && c <= 122)
                out += String.fromCodePoint(0x1D44E + c - 97);
            else if (c >= 65 && c <= 90)
                out += String.fromCodePoint(0x1D434 + c - 65);
            else
                out += ch;
        }
        return out;
    }

    FontMetrics {
        id: metrics
        font.family: root.family
        font.pixelSize: root.size
    }

    Loader {
        id: body
        sourceComponent: ({
                text: textPart,
                var: textPart,
                row: rowPart,
                frac: fracPart,
                sqrt: sqrtPart,
                sup: supPart,
                sub: subPart,
                delim: delimPart,
                mat: matPart
            })[root.kind]
    }

    // A nested piece: this file again, following the given node and size.
    component Part: Loader {
        id: part

        property var node: ""
        property real size: 32
        property color color: "white"
        property int level: 0
        readonly property real axis: item ? item.axis : 0

        source: Qt.resolvedUrl("Formula.qml")
        onLoaded: {
            item.node = Qt.binding(() => part.node);
            item.size = Qt.binding(() => part.size);
            item.color = Qt.binding(() => part.color);
            item.level = Qt.binding(() => part.level);
        }
    }

    // A bracket drawn to a height: ( ) as tapered curves, [ ] and | ‖ as strokes, mirrored for
    // the closing ones; anything else as the font's glyph, stretched.
    component Fence: Item {
        id: fence

        property string glyph: "("
        property real size: 32
        property real rule: 1.5
        property color color: "white"
        property string family: ""
        readonly property bool paren: glyph === "(" || glyph === ")"
        readonly property bool square: glyph === "[" || glyph === "]"
        readonly property bool bar: glyph === "|" || glyph === "‖"
        readonly property bool closing: glyph === ")" || glyph === "]"

        implicitWidth: glyph === "" ? 0 : glyph === "|" ? size * 0.25 : size * 0.38

        // Drawn as for an opening bracket; closing ones are mirrored.
        Item {
            anchors.fill: parent
            transform: Scale {
                origin.x: fence.width / 2
                xScale: fence.closing ? -1 : 1
            }

            // A crescent: the outer curve and an inner one bowed a little less, filled, so the
            // stroke is thickest in the middle like a typeset parenthesis.
            Shape {
                anchors.fill: parent
                visible: fence.paren
                preferredRendererType: Shape.CurveRenderer

                ShapePath {
                    id: crescent

                    readonly property real w: fence.width
                    readonly property real h: fence.height
                    readonly property real thick: fence.rule * 1.7

                    strokeColor: "transparent"
                    fillColor: fence.color
                    startX: w * 0.85
                    startY: 0
                    PathQuad {
                        x: crescent.w * 0.85
                        y: crescent.h
                        controlX: -crescent.w * 0.15
                        controlY: crescent.h / 2
                    }
                    PathQuad {
                        x: crescent.w * 0.85
                        y: 0
                        controlX: -crescent.w * 0.15 + 2 * crescent.thick
                        controlY: crescent.h / 2
                    }
                }
            }
            Shape {
                anchors.fill: parent
                visible: fence.square
                preferredRendererType: Shape.CurveRenderer

                ShapePath {
                    id: corners

                    readonly property real w: fence.width
                    readonly property real h: fence.height
                    readonly property real half: fence.rule * 0.55

                    strokeColor: fence.color
                    strokeWidth: fence.rule * 1.1
                    fillColor: "transparent"
                    joinStyle: ShapePath.MiterJoin
                    capStyle: ShapePath.FlatCap
                    startX: w * 0.8
                    startY: half
                    PathLine {
                        x: corners.w * 0.3
                        y: corners.half
                    }
                    PathLine {
                        x: corners.w * 0.3
                        y: corners.h - corners.half
                    }
                    PathLine {
                        x: corners.w * 0.8
                        y: corners.h - corners.half
                    }
                }
            }
        }
        Rectangle {
            visible: fence.bar
            x: fence.glyph === "|" ? (fence.width - width) / 2 : fence.width * 0.25
            width: fence.rule * 1.1
            height: fence.height
            color: fence.color
        }
        Rectangle {
            visible: fence.glyph === "‖"
            x: fence.width * 0.65
            width: fence.rule * 1.1
            height: fence.height
            color: fence.color
        }
        Text {
            visible: !fence.paren && !fence.square && !fence.bar && fence.glyph !== ""
            anchors.centerIn: parent
            text: fence.glyph
            color: fence.color
            font.family: fence.family
            font.pixelSize: fence.size
            transform: Scale {
                origin.y: fence.height / 2
                yScale: Math.max(1, fence.height / Math.max(1, fence.size * 1.2))
            }
        }
    }

    Component {
        id: textPart

        Text {
            readonly property real axis: root.textAxis

            text: root.kind === "var" ? root.italic(root.node.v) : String(root.node ?? "")
            color: root.color
            font.family: root.family
            font.pixelSize: root.size
        }
    }

    // Side by side, each piece's axis on the row's.
    Component {
        id: rowPart

        Item {
            id: line

            readonly property real axis: {
                let m = 0;
                for (let i = 0; i < row.children.length; i++)
                    m = Math.max(m, row.children[i].axis ?? 0);
                return m;
            }
            readonly property real below: {
                let m = 0;
                for (let i = 0; i < row.children.length; i++) {
                    const c = row.children[i];
                    if (c.axis !== undefined)
                        m = Math.max(m, c.implicitHeight - c.axis);
                }
                return m;
            }

            implicitWidth: row.implicitWidth
            implicitHeight: axis + below

            Row {
                id: row

                Repeater {
                    model: root.kind === "row" ? Array.from(root.node) : []

                    Part {
                        required property var modelData

                        y: line.axis - axis
                        node: modelData
                        size: root.size
                        color: root.color
                        level: root.level
                    }
                }
            }
        }
    }

    // Numerator over denominator, the bar on the axis. Inside another fraction (or a script)
    // the parts get smaller.
    Component {
        id: fracPart

        Item {
            id: frac

            readonly property real gap: root.size * 0.14
            readonly property real pad: root.size * 0.1
            readonly property real partSize: root.level === 0 ? root.size : root.size * 0.78
            readonly property real axis: num.implicitHeight + gap + root.rule / 2

            implicitWidth: Math.max(num.implicitWidth, den.implicitWidth) + 2 * pad
            implicitHeight: num.implicitHeight + 2 * gap + root.rule + den.implicitHeight

            Part {
                id: num
                x: (frac.implicitWidth - implicitWidth) / 2
                node: root.node.frac[0]
                size: frac.partSize
                color: root.color
                level: root.level + 1
            }
            Rectangle {
                y: num.implicitHeight + frac.gap
                width: frac.implicitWidth
                height: root.rule
                color: root.color
            }
            Part {
                id: den
                x: (frac.implicitWidth - implicitWidth) / 2
                y: num.implicitHeight + 2 * frac.gap + root.rule
                node: root.node.frac[1]
                size: frac.partSize
                color: root.color
                level: root.level + 1
            }
        }
    }

    // The radical: a small tick, down to the bottom, up to the bar over the contents. An index
    // (cube roots) sits small in the crook.
    Component {
        id: sqrtPart

        Item {
            id: radical

            readonly property bool hasIndex: root.node.n !== undefined
            readonly property real gap: root.size * 0.12
            readonly property real above: root.rule + gap
            readonly property real sign: root.size * 0.5 + body2.implicitHeight * 0.08
            readonly property real indexWidth: hasIndex ? Math.max(0, index.implicitWidth - sign * 0.35) : 0
            readonly property real axis: above + body2.axis

            implicitWidth: indexWidth + sign + body2.implicitWidth + root.size * 0.1
            implicitHeight: above + body2.implicitHeight + root.size * 0.04

            Shape {
                anchors.fill: parent
                preferredRendererType: Shape.CurveRenderer

                ShapePath {
                    strokeColor: root.color
                    strokeWidth: root.rule
                    fillColor: "transparent"
                    joinStyle: ShapePath.MiterJoin
                    capStyle: ShapePath.FlatCap

                    startX: radical.indexWidth
                    startY: radical.implicitHeight * 0.62
                    PathLine {
                        x: radical.indexWidth + radical.sign * 0.22
                        y: radical.implicitHeight * 0.54
                    }
                    PathLine {
                        x: radical.indexWidth + radical.sign * 0.5
                        y: radical.implicitHeight - root.rule
                    }
                    PathLine {
                        x: radical.indexWidth + radical.sign
                        y: root.rule / 2
                    }
                    PathLine {
                        x: radical.implicitWidth
                        y: root.rule / 2
                    }
                }
            }
            Part {
                id: index
                visible: radical.hasIndex
                y: radical.implicitHeight * 0.5 - implicitHeight
                node: radical.hasIndex ? root.node.n : ""
                size: root.size * 0.55
                color: root.color
                level: root.level + 1
            }
            Part {
                id: body2
                x: radical.indexWidth + radical.sign + root.size * 0.05
                y: radical.above
                node: root.node.sqrt
                size: root.size
                color: root.color
                level: root.level
            }
        }
    }

    // A power: smaller, raised so its axis sits a third of an em above the base's (or near
    // the top of a tall base).
    Component {
        id: supPart

        Item {
            id: power

            // How far the base reaches above its axis beyond a line of text, plus a third of an em.
            readonly property real shift: root.size * 0.33 + Math.max(0, base.axis - root.textAxis)
            readonly property real lift: exp.axis + shift - base.axis
            readonly property real baseY: Math.max(0, lift)
            readonly property real expY: Math.max(0, -lift)
            readonly property real axis: baseY + base.axis

            implicitWidth: base.implicitWidth + root.size * 0.04 + exp.implicitWidth
            implicitHeight: Math.max(baseY + base.implicitHeight, expY + exp.implicitHeight)

            Part {
                id: base
                y: power.baseY
                node: root.node.sup[0]
                size: root.size
                color: root.color
                level: root.level
            }
            Part {
                id: exp
                x: base.implicitWidth + root.size * 0.04
                y: power.expY
                node: root.node.sup[1]
                size: root.size * 0.7
                color: root.color
                level: root.level + 1
            }
        }
    }

    // A subscript: smaller, dropped below the base's axis.
    Component {
        id: subPart

        Item {
            id: lower

            readonly property real subY: base2.axis + root.size * 0.35 - script.axis
            readonly property real axis: base2.axis

            implicitWidth: base2.implicitWidth + root.size * 0.03 + script.implicitWidth
            implicitHeight: Math.max(base2.implicitHeight, subY + script.implicitHeight)

            Part {
                id: base2
                node: root.node.sub[0]
                size: root.size
                color: root.color
                level: root.level
            }
            Part {
                id: script
                x: base2.implicitWidth + root.size * 0.03
                y: lower.subY
                node: root.node.sub[1]
                size: root.size * 0.7
                color: root.color
                level: root.level + 1
            }
        }
    }

    // Brackets around something: the font's own when the contents are a line high, drawn to
    // fit when they're taller (a fraction, a matrix).
    Component {
        id: delimPart

        Item {
            id: fenced

            readonly property bool tall: inner.implicitHeight > root.lineHeight * 1.15
            readonly property real fenceHeight: Math.max(root.lineHeight, inner.implicitHeight + root.size * 0.1)
            readonly property real axis: tall ? Math.max(inner.axis, fenceHeight / 2) : root.textAxis
            readonly property real innerY: axis - inner.axis
            readonly property real fenceY: tall ? axis - fenceHeight / 2 : 0

            implicitWidth: open.implicitWidth + inner.implicitWidth + close.implicitWidth
            implicitHeight: Math.max(fenceY + (tall ? fenceHeight : root.lineHeight), innerY + inner.implicitHeight)

            Text {
                id: open
                visible: !fenced.tall
                text: root.node.delim[0]
                color: root.color
                font.family: root.family
                font.pixelSize: root.size
            }
            Fence {
                visible: fenced.tall
                y: fenced.fenceY
                width: open.implicitWidth
                height: fenced.fenceHeight
                glyph: root.node.delim[0]
                size: root.size
                rule: root.rule
                color: root.color
                family: root.family
            }
            Part {
                id: inner
                x: open.implicitWidth
                y: fenced.innerY
                node: root.node.delim[1]
                size: root.size
                color: root.color
                level: root.level
            }
            Text {
                id: close
                visible: !fenced.tall
                x: open.implicitWidth + inner.implicitWidth
                text: root.node.delim[2]
                color: root.color
                font.family: root.family
                font.pixelSize: root.size
            }
            Fence {
                visible: fenced.tall
                x: close.x
                y: fenced.fenceY
                width: close.implicitWidth
                height: fenced.fenceHeight
                glyph: root.node.delim[2]
                size: root.size
                rule: root.rule
                color: root.color
                family: root.family
            }
        }
    }

    // A matrix: cells centred in their columns, rows lined up on their axes, between drawn
    // brackets; its middle sits on the axis.
    Component {
        id: matPart

        Item {
            id: matrix

            readonly property var rows: Array.from(root.node.mat).map(r => Array.from(r))
            readonly property int columns: Math.max(...rows.map(r => r.length), 0)
            readonly property var cells: [].concat(...rows.map((r, i) => r.map((node, j) => ({
                            node: node,
                            row: i,
                            col: j
                        }))))
            readonly property real colGap: root.size * 0.9
            readonly property real rowGap: root.size * 0.3
            readonly property real fenceWidth: root.node.l ? root.size * 0.38 : 0
            readonly property real pad: root.node.l ? root.size * 0.15 : 0

            // Column widths, and each row's height above and below its axis.
            readonly property var layout: {
                const widths = new Array(columns).fill(0);
                const above = new Array(rows.length).fill(0);
                const below = new Array(rows.length).fill(0);
                for (let i = 0; i < grid.count; i++) {
                    const item = grid.itemAt(i);
                    if (!item)
                        continue;
                    const c = cells[i];
                    widths[c.col] = Math.max(widths[c.col], item.implicitWidth);
                    above[c.row] = Math.max(above[c.row], item.axis);
                    below[c.row] = Math.max(below[c.row], item.implicitHeight - item.axis);
                }
                const xs = [];
                let x = fenceWidth + pad;
                for (const w of widths) {
                    xs.push(x);
                    x += w + colGap;
                }
                const ys = [];
                let y = 0;
                for (let r = 0; r < rows.length; r++) {
                    ys.push(y + above[r]);
                    y += above[r] + below[r] + rowGap;
                }
                return {
                    widths: widths,
                    xs: xs,
                    axes: ys,
                    width: x - colGap + pad + fenceWidth,
                    height: Math.max(0, y - rowGap)
                };
            }
            readonly property real axis: layout.height / 2

            implicitWidth: layout.width
            implicitHeight: layout.height

            Repeater {
                id: grid
                model: matrix.cells

                Part {
                    required property var modelData

                    x: (matrix.layout.xs[modelData.col] ?? 0) + ((matrix.layout.widths[modelData.col] ?? 0) - implicitWidth) / 2
                    y: (matrix.layout.axes[modelData.row] ?? 0) - axis
                    node: modelData.node
                    size: root.size
                    color: root.color
                    level: root.level
                }
            }
            Fence {
                width: matrix.fenceWidth
                height: matrix.layout.height
                glyph: root.node.l ?? ""
                size: root.size
                rule: root.rule
                color: root.color
                family: root.family
            }
            Fence {
                x: matrix.layout.width - matrix.fenceWidth
                width: matrix.fenceWidth
                height: matrix.layout.height
                glyph: root.node.r ?? ""
                size: root.size
                rule: root.rule
                color: root.color
                family: root.family
            }
        }
    }
}
