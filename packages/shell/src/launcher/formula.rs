//! A formula set the way TeX would (Formula.qml): fractions stacked, roots under a radical,
//! powers raised, everything lined up on the math axis (the height of a fraction bar), in New
//! Computer Modern Math. The tree comes from `services::launcher::find`, which makes it from
//! qalc's LaTeX:
//!
//!   "text" | [a, b, ...] | { v: "x" } | { frac: [a, b] } | { sqrt: a, n?: b } | { sup: [a, b] }
//!   | { sub: [a, b] } | { delim: ["(", a, ")"] } | { mat: [[a, ...], ...], l: "[", r: "]" }
//!
//! Laid out here into marks (text, rules, strokes) with the same geometry as the QML, then
//! painted on a canvas. Each piece tells its parent how far down from its top the axis is, so
//! a row can line them up.

use gpui::{
    AnyElement, App, Bounds, Font, Pixels, Rgba, TextAlign, TextRun, Window, canvas, div, point,
    prelude::*, px,
};
use serde_json::Value;

const FAMILY: &str = "NewComputerModern Math";

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text(String),
    /// A variable, set in math italics.
    Var(String),
    Row(Vec<Node>),
    Frac(Box<Node>, Box<Node>),
    /// The radicand and an index (cube roots).
    Sqrt(Box<Node>, Option<Box<Node>>),
    Sup(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Delim(String, Box<Node>, String),
    Mat(Vec<Vec<Node>>, String, String),
}

impl Node {
    pub fn parse(v: &Value) -> Node {
        let pair = |v: Option<&Value>| match v {
            Some(Value::Array(p)) if p.len() == 2 => {
                Some((Box::new(Self::parse(&p[0])), Box::new(Self::parse(&p[1]))))
            }
            _ => None,
        };
        let string = |v: Option<&Value>| match v {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        match v {
            Value::String(s) => Node::Text(s.clone()),
            Value::Number(n) => Node::Text(n.to_string()),
            Value::Array(items) => Node::Row(items.iter().map(Self::parse).collect()),
            Value::Object(o) => {
                if let Some(v) = o.get("v") {
                    Node::Var(string(Some(v)))
                } else if let Some((a, b)) = pair(o.get("frac")) {
                    Node::Frac(a, b)
                } else if let Some(a) = o.get("sqrt") {
                    Node::Sqrt(
                        Box::new(Self::parse(a)),
                        o.get("n").map(|n| Box::new(Self::parse(n))),
                    )
                } else if let Some((a, b)) = pair(o.get("sup")) {
                    Node::Sup(a, b)
                } else if let Some((a, b)) = pair(o.get("sub")) {
                    Node::Sub(a, b)
                } else if let Some(Value::Array(d)) = o.get("delim")
                    && d.len() == 3
                {
                    Node::Delim(
                        string(d.first()),
                        Box::new(Self::parse(&d[1])),
                        string(d.get(2)),
                    )
                } else if let Some(Value::Array(rows)) = o.get("mat") {
                    let rows = rows
                        .iter()
                        .map(|r| match r {
                            Value::Array(cells) => cells.iter().map(Self::parse).collect(),
                            other => vec![Self::parse(other)],
                        })
                        .collect();
                    Node::Mat(rows, string(o.get("l")), string(o.get("r")))
                } else {
                    Node::Text(String::new())
                }
            }
            _ => Node::Text(String::new()),
        }
    }
}

/// Letters as the Unicode math italics (𝑥, 𝑛), which is how math fonts carry them.
pub fn italic(s: &str) -> String {
    s.chars()
        .map(|ch| match ch {
            'h' => 'ℎ',
            'a'..='z' => char::from_u32(0x1D44E + ch as u32 - 'a' as u32).unwrap_or(ch),
            'A'..='Z' => char::from_u32(0x1D434 + ch as u32 - 'A' as u32).unwrap_or(ch),
            _ => ch,
        })
        .collect()
}

/// Font measurements, in pixels at `size`.
pub trait Metrics {
    fn width(&self, text: &str, size: f32) -> f32;
    fn ascent(&self, size: f32) -> f32;
    fn descent(&self, size: f32) -> f32;
    fn x_height(&self, size: f32) -> f32;

    /// From the top of a line of text down to the axis: half an x-height above the baseline.
    fn text_axis(&self, size: f32) -> f32 {
        self.ascent(size) - self.x_height(size) / 2.
    }

    fn line_height(&self, size: f32) -> f32 {
        self.ascent(size) + self.descent(size)
    }
}

/// Something to draw, relative to its box's top left.
#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    /// A line of text, its top at `y`.
    Text {
        x: f32,
        y: f32,
        size: f32,
        text: String,
    },
    /// A filled rectangle: fraction bars, the strokes of | and ‖.
    Rule { x: f32, y: f32, w: f32, h: f32 },
    /// A polyline: the radical, square brackets.
    Stroke { points: Vec<(f32, f32)>, width: f32 },
    /// A parenthesis drawn to a height: a filled crescent, thickest in the middle.
    Paren {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        thick: f32,
        closing: bool,
    },
}

impl Mark {
    fn offset(mut self, dx: f32, dy: f32) -> Self {
        match &mut self {
            Mark::Text { x, y, .. } | Mark::Rule { x, y, .. } | Mark::Paren { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Mark::Stroke { points, .. } => {
                for (x, y) in points {
                    *x += dx;
                    *y += dy;
                }
            }
        }
        self
    }
}

/// A laid out piece: its size, where its axis is, and what to draw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Laid {
    pub width: f32,
    pub height: f32,
    pub axis: f32,
    pub marks: Vec<Mark>,
}

impl Laid {
    fn place(&mut self, child: Laid, x: f32, y: f32) {
        self.marks
            .extend(child.marks.into_iter().map(|m| m.offset(x, y)));
    }
}

/// Line thickness: fraction bars, the radical, drawn brackets.
fn rule(size: f32) -> f32 {
    (size * 0.045).max(1.)
}

fn text(s: String, size: f32, m: &dyn Metrics) -> Laid {
    Laid {
        width: m.width(&s, size),
        height: m.line_height(size),
        axis: m.text_axis(size),
        marks: vec![Mark::Text {
            x: 0.,
            y: 0.,
            size,
            text: s,
        }],
    }
}

/// A bracket drawn to a height: ( ) as crescents, [ ] and | ‖ as strokes, closing ones
/// mirrored; anything else as the font's glyph, centred.
fn fence(glyph: &str, w: f32, h: f32, size: f32, m: &dyn Metrics) -> Vec<Mark> {
    let rule = rule(size);
    let closing = matches!(glyph, ")" | "]");
    // Drawn as for an opening bracket; closing ones are mirrored.
    let mirror = |x: f32| if closing { w - x } else { x };
    match glyph {
        "" => Vec::new(),
        "(" | ")" => vec![Mark::Paren {
            x: 0.,
            y: 0.,
            w,
            h,
            thick: rule * 1.7,
            closing,
        }],
        "[" | "]" => {
            let half = rule * 0.55;
            vec![Mark::Stroke {
                points: vec![
                    (mirror(w * 0.8), half),
                    (mirror(w * 0.3), half),
                    (mirror(w * 0.3), h - half),
                    (mirror(w * 0.8), h - half),
                ],
                width: rule * 1.1,
            }]
        }
        "|" => vec![Mark::Rule {
            x: (w - rule * 1.1) / 2.,
            y: 0.,
            w: rule * 1.1,
            h,
        }],
        "‖" => vec![
            Mark::Rule {
                x: w * 0.25,
                y: 0.,
                w: rule * 1.1,
                h,
            },
            Mark::Rule {
                x: w * 0.65,
                y: 0.,
                w: rule * 1.1,
                h,
            },
        ],
        other => {
            let gw = m.width(other, size);
            vec![Mark::Text {
                x: (w - gw) / 2.,
                y: (h - m.line_height(size)) / 2.,
                size,
                text: other.to_string(),
            }]
        }
    }
}

/// Lays out `node` at `size`; `level` is how many fractions or scripts deep it is.
pub fn layout(node: &Node, size: f32, level: u32, m: &dyn Metrics) -> Laid {
    match node {
        Node::Text(s) => text(s.clone(), size, m),
        Node::Var(v) => text(italic(v), size, m),
        Node::Row(items) => {
            let parts: Vec<Laid> = items.iter().map(|n| layout(n, size, level, m)).collect();
            let axis = parts.iter().map(|p| p.axis).fold(0., f32::max);
            let below = parts.iter().map(|p| p.height - p.axis).fold(0., f32::max);
            let mut out = Laid {
                axis,
                height: axis + below,
                ..Laid::default()
            };
            for part in parts {
                let (x, y) = (out.width, axis - part.axis);
                out.width += part.width;
                out.place(part, x, y);
            }
            out
        }
        Node::Frac(a, b) => {
            // Inside another fraction (or a script) the parts get smaller.
            let gap = size * 0.14;
            let pad = size * 0.1;
            let part_size = if level == 0 { size } else { size * 0.78 };
            let num = layout(a, part_size, level + 1, m);
            let den = layout(b, part_size, level + 1, m);
            let rule = rule(size);
            let width = num.width.max(den.width) + 2. * pad;
            let mut out = Laid {
                width,
                height: num.height + 2. * gap + rule + den.height,
                axis: num.height + gap + rule / 2.,
                marks: vec![Mark::Rule {
                    x: 0.,
                    y: num.height + gap,
                    w: width,
                    h: rule,
                }],
            };
            let (num_x, den_x) = ((width - num.width) / 2., (width - den.width) / 2.);
            let den_y = num.height + 2. * gap + rule;
            out.place(num, num_x, 0.);
            out.place(den, den_x, den_y);
            out
        }
        Node::Sqrt(a, n) => {
            // A small tick, down to the bottom, up to the bar over the contents. An index sits
            // small in the crook.
            let rule = rule(size);
            let body = layout(a, size, level, m);
            let index = n.as_ref().map(|n| layout(n, size * 0.55, level + 1, m));
            let gap = size * 0.12;
            let above = rule + gap;
            let sign = size * 0.5 + body.height * 0.08;
            let iw = index
                .as_ref()
                .map_or(0., |i| (i.width - sign * 0.35).max(0.));
            let width = iw + sign + body.width + size * 0.1;
            let height = above + body.height + size * 0.04;
            let mut out = Laid {
                width,
                height,
                axis: above + body.axis,
                marks: vec![Mark::Stroke {
                    points: vec![
                        (iw, height * 0.62),
                        (iw + sign * 0.22, height * 0.54),
                        (iw + sign * 0.5, height - rule),
                        (iw + sign, rule / 2.),
                        (width, rule / 2.),
                    ],
                    width: rule,
                }],
            };
            if let Some(index) = index {
                let y = height * 0.5 - index.height;
                out.place(index, 0., y);
            }
            out.place(body, iw + sign + size * 0.05, above);
            out
        }
        Node::Sup(a, b) => {
            // Smaller, raised so its axis sits a third of an em above the base's (or near the
            // top of a tall base).
            let base = layout(a, size, level, m);
            let exp = layout(b, size * 0.7, level + 1, m);
            let shift = size * 0.33 + (base.axis - m.text_axis(size)).max(0.);
            let lift = exp.axis + shift - base.axis;
            let (base_y, exp_y) = (lift.max(0.), (-lift).max(0.));
            let mut out = Laid {
                width: base.width + size * 0.04 + exp.width,
                height: (base_y + base.height).max(exp_y + exp.height),
                axis: base_y + base.axis,
                marks: Vec::new(),
            };
            let exp_x = base.width + size * 0.04;
            out.place(base, 0., base_y);
            out.place(exp, exp_x, exp_y);
            out
        }
        Node::Sub(a, b) => {
            // Smaller, dropped below the base's axis.
            let base = layout(a, size, level, m);
            let script = layout(b, size * 0.7, level + 1, m);
            let sub_y = base.axis + size * 0.35 - script.axis;
            let mut out = Laid {
                width: base.width + size * 0.03 + script.width,
                height: base.height.max(sub_y + script.height),
                axis: base.axis,
                marks: Vec::new(),
            };
            let x = base.width + size * 0.03;
            out.place(base, 0., 0.);
            out.place(script, x, sub_y);
            out
        }
        Node::Delim(l, a, r) => {
            // The font's own brackets when the contents are a line high, drawn to fit when
            // they're taller (a fraction, a matrix).
            let inner = layout(a, size, level, m);
            let line = m.line_height(size);
            let (ow, cw) = (m.width(l, size), m.width(r, size));
            let tall = inner.height > line * 1.15;
            let fence_h = line.max(inner.height + size * 0.1);
            let axis = if tall {
                inner.axis.max(fence_h / 2.)
            } else {
                m.text_axis(size)
            };
            let inner_y = axis - inner.axis;
            let fence_y = if tall { axis - fence_h / 2. } else { 0. };
            let mut out = Laid {
                width: ow + inner.width + cw,
                height: (fence_y + if tall { fence_h } else { line }).max(inner_y + inner.height),
                axis,
                marks: Vec::new(),
            };
            let close_x = ow + inner.width;
            if tall {
                for mark in fence(l, ow, fence_h, size, m) {
                    out.marks.push(mark.offset(0., fence_y));
                }
                for mark in fence(r, cw, fence_h, size, m) {
                    out.marks.push(mark.offset(close_x, fence_y));
                }
            } else {
                out.place(text(l.clone(), size, m), 0., 0.);
                out.place(text(r.clone(), size, m), close_x, 0.);
            }
            out.place(inner, ow, inner_y);
            out
        }
        Node::Mat(rows, l, r) => {
            // Cells centred in their columns, rows lined up on their axes, between drawn
            // brackets; its middle sits on the axis.
            let cells: Vec<Vec<Laid>> = rows
                .iter()
                .map(|row| row.iter().map(|c| layout(c, size, level, m)).collect())
                .collect();
            let columns = cells.iter().map(Vec::len).max().unwrap_or(0);
            let col_gap = size * 0.9;
            let row_gap = size * 0.3;
            let fence_w = if l.is_empty() { 0. } else { size * 0.38 };
            let pad = if l.is_empty() { 0. } else { size * 0.15 };
            let mut widths = vec![0f32; columns];
            let mut above = vec![0f32; cells.len()];
            let mut below = vec![0f32; cells.len()];
            for (r, row) in cells.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    widths[c] = widths[c].max(cell.width);
                    above[r] = above[r].max(cell.axis);
                    below[r] = below[r].max(cell.height - cell.axis);
                }
            }
            let mut xs = Vec::with_capacity(columns);
            let mut x = fence_w + pad;
            for w in &widths {
                xs.push(x);
                x += w + col_gap;
            }
            let mut axes = Vec::with_capacity(cells.len());
            let mut y = 0.;
            for r in 0..cells.len() {
                axes.push(y + above[r]);
                y += above[r] + below[r] + row_gap;
            }
            let width = x - col_gap + pad + fence_w;
            let height = (y - row_gap).max(0.);
            let mut out = Laid {
                width,
                height,
                axis: height / 2.,
                marks: Vec::new(),
            };
            for (r, row) in cells.into_iter().enumerate() {
                for (c, cell) in row.into_iter().enumerate() {
                    let cx = xs[c] + (widths[c] - cell.width) / 2.;
                    let cy = axes[r] - cell.axis;
                    out.place(cell, cx, cy);
                }
            }
            out.marks.extend(fence(l, fence_w, height, size, m));
            for mark in fence(r, fence_w, height, size, m) {
                out.marks.push(mark.offset(width - fence_w, 0.));
            }
            out
        }
    }
}

/// gpui's measurements of the math font.
struct FontMetrics<'a> {
    window: &'a Window,
    font: Font,
}

impl FontMetrics<'_> {
    fn font_id(&self) -> gpui::FontId {
        self.window.text_system().resolve_font(&self.font)
    }
}

impl Metrics for FontMetrics<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        if text.is_empty() {
            return 0.;
        }
        let run = TextRun {
            len: text.len(),
            font: self.font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let layout = self
            .window
            .text_system()
            .layout_line(text, px(size), &[run], None);
        f32::from(layout.width)
    }

    fn ascent(&self, size: f32) -> f32 {
        f32::from(self.window.text_system().ascent(self.font_id(), px(size))).abs()
    }

    fn descent(&self, size: f32) -> f32 {
        f32::from(self.window.text_system().descent(self.font_id(), px(size))).abs()
    }

    fn x_height(&self, size: f32) -> f32 {
        f32::from(self.window.text_system().x_height(self.font_id(), px(size))).abs()
    }
}

/// `node` typeset at `size` in `color`, shrunk to fit `max_width`.
pub fn view(node: &Node, size: f32, color: Rgba, max_width: f32, window: &Window) -> AnyElement {
    let metrics = FontMetrics {
        window,
        font: gpui::font(FAMILY),
    };
    let mut laid = layout(node, size, 0, &metrics);
    if laid.width > max_width && laid.width > 0. {
        laid = layout(node, size * max_width / laid.width, 0, &metrics);
    }
    let (width, height) = (laid.width, laid.height);
    let marks = laid.marks;
    let font = metrics.font.clone();
    div()
        .relative()
        .flex_none()
        .w(px(width.ceil()))
        .h(px(height.ceil()))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| paint(&marks, &font, color, bounds, window, cx),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

fn paint(
    marks: &[Mark],
    font: &Font,
    color: Rgba,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let at = |x: f32, y: f32| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    for mark in marks {
        match mark {
            Mark::Text { x, y, size, text } => {
                if text.is_empty() {
                    continue;
                }
                let run = TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: color.into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let line =
                    window
                        .text_system()
                        .shape_line(text.clone().into(), px(*size), &[run], None);
                let id = window.text_system().resolve_font(font);
                let ascent = f32::from(window.text_system().ascent(id, px(*size))).abs();
                let descent = f32::from(window.text_system().descent(id, px(*size))).abs();
                let _ = line.paint(
                    at(*x, *y),
                    px(ascent + descent),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            Mark::Rule { x, y, w, h } => {
                window.paint_quad(gpui::fill(
                    Bounds::new(at(*x, *y), gpui::size(px(*w), px(*h))),
                    color,
                ));
            }
            Mark::Stroke { points, width } => {
                let mut path = gpui::PathBuilder::stroke(px(*width));
                for (i, (x, y)) in points.iter().enumerate() {
                    if i == 0 {
                        path.move_to(at(*x, *y));
                    } else {
                        path.line_to(at(*x, *y));
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
            Mark::Paren {
                x,
                y,
                w,
                h,
                thick,
                closing,
            } => {
                // The outer curve, then back up an inner one bowed a little less.
                let mx = |px_: f32| x + if *closing { w - px_ } else { px_ };
                let p = |px_: f32, py: f32| at(mx(px_), y + py);
                let mut path = gpui::PathBuilder::fill();
                path.move_to(p(w * 0.85, 0.));
                path.curve_to(p(w * 0.85, *h), p(-w * 0.15, h / 2.));
                path.curve_to(p(w * 0.85, 0.), p(-w * 0.15 + 2. * thick, h / 2.));
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every character 0.5 em wide; ascent 0.8, descent 0.2, x-height 0.4 em.
    struct Fixed;

    impl Metrics for Fixed {
        fn width(&self, text: &str, size: f32) -> f32 {
            text.chars().count() as f32 * size * 0.5
        }
        fn ascent(&self, size: f32) -> f32 {
            size * 0.8
        }
        fn descent(&self, size: f32) -> f32 {
            size * 0.2
        }
        fn x_height(&self, size: f32) -> f32 {
            size * 0.4
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn parsing() {
        let tree = json!([{"sup":[{"v":"x"},"2"]}," + ",{"frac":[{"sqrt":"2"},"3"]}]);
        let node = Node::parse(&tree);
        let Node::Row(items) = &node else {
            panic!("not a row");
        };
        assert_eq!(items.len(), 3);
        assert_eq!(
            items[0],
            Node::Sup(
                Box::new(Node::Var("x".into())),
                Box::new(Node::Text("2".into()))
            )
        );
        assert!(matches!(&items[2], Node::Frac(a, _) if matches!(**a, Node::Sqrt(_, None))));

        let mat = Node::parse(&json!({"l":"[","mat":[["1","2"],["3","4"]],"r":"]"}));
        assert!(matches!(&mat, Node::Mat(rows, l, r) if rows.len() == 2 && l == "[" && r == "]"));
        let delim = Node::parse(&json!({"delim":["(", "a", ")"]}));
        assert!(matches!(delim, Node::Delim(ref l, _, ref r) if l == "(" && r == ")"));
        let cube = Node::parse(&json!({"sqrt":"8","n":"3"}));
        assert!(matches!(cube, Node::Sqrt(_, Some(_))));
    }

    #[test]
    fn italics() {
        assert_eq!(italic("x"), "𝑥");
        assert_eq!(italic("h"), "ℎ");
        assert_eq!(italic("A1"), "𝐴1");
    }

    #[test]
    fn text_and_rows() {
        let laid = layout(&Node::Text("12".into()), 20., 0, &Fixed);
        assert!(close(laid.width, 20.) && close(laid.height, 20.) && close(laid.axis, 12.));

        // A row lines its parts up on the axis: the fraction's axis is lower than the text's.
        let row = Node::Row(vec![
            Node::Text("a".into()),
            Node::Frac(
                Box::new(Node::Text("1".into())),
                Box::new(Node::Text("2".into())),
            ),
        ]);
        let laid = layout(&row, 20., 0, &Fixed);
        // The rule is at least a pixel.
        let frac_axis = 20. + 20. * 0.14 + 1. / 2.;
        assert!(close(laid.axis, frac_axis));
        let Mark::Text { y, .. } = &laid.marks[0] else {
            panic!("text first");
        };
        assert!(close(*y, frac_axis - 12.));
    }

    #[test]
    fn fractions() {
        let frac = Node::Frac(
            Box::new(Node::Text("1".into())),
            Box::new(Node::Text("234".into())),
        );
        let laid = layout(&frac, 20., 0, &Fixed);
        // As wide as the wider part plus padding; the bar spans it.
        assert!(close(laid.width, 30. + 4.));
        assert!(
            laid.marks
                .iter()
                .any(|m| matches!(m, Mark::Rule { w, .. } if close(*w, 34.)))
        );
        // Nested, the parts shrink.
        let nested = layout(&frac, 20., 1, &Fixed);
        assert!(nested.width < laid.width);
    }

    #[test]
    fn scripts_and_roots() {
        let sup = Node::Sup(
            Box::new(Node::Text("x".into())),
            Box::new(Node::Text("2".into())),
        );
        let laid = layout(&sup, 20., 0, &Fixed);
        // The exponent sits above: the base moves down, and the box is taller than a line.
        assert!(laid.axis > 12. && laid.height > 20.);

        let root = Node::Sqrt(Box::new(Node::Text("2".into())), None);
        let laid = layout(&root, 20., 0, &Fixed);
        assert!(
            laid.marks
                .iter()
                .any(|m| matches!(m, Mark::Stroke { points, .. } if points.len() == 5))
        );
        assert!(laid.width > 10.);
    }

    #[test]
    fn brackets() {
        // A line high: the font's brackets.
        let short = Node::Delim("(".into(), Box::new(Node::Text("a".into())), ")".into());
        let laid = layout(&short, 20., 0, &Fixed);
        assert_eq!(
            laid.marks
                .iter()
                .filter(|m| matches!(m, Mark::Text { .. }))
                .count(),
            3
        );

        // Around a fraction: drawn to fit, the closing one mirrored.
        let tall = Node::Delim(
            "(".into(),
            Box::new(Node::Frac(
                Box::new(Node::Text("1".into())),
                Box::new(Node::Text("2".into())),
            )),
            ")".into(),
        );
        let laid = layout(&tall, 20., 0, &Fixed);
        let parens: Vec<bool> = laid
            .marks
            .iter()
            .filter_map(|m| match m {
                Mark::Paren { closing, .. } => Some(*closing),
                _ => None,
            })
            .collect();
        assert_eq!(parens, vec![false, true]);

        let mat = Node::parse(&json!({"l":"[","mat":[["1","2"],["3","4"]],"r":"]"}));
        let laid = layout(&mat, 20., 0, &Fixed);
        assert!(close(laid.axis, laid.height / 2.));
        assert_eq!(
            laid.marks
                .iter()
                .filter(|m| matches!(m, Mark::Stroke { .. }))
                .count(),
            2
        );
    }
}
