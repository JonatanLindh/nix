//! qalc's LaTeX (`qalc -latex`) as a tree the launcher lays out itself (Math.qml), so results
//! read like TeX would set them: stacked fractions, radicals, raised powers.
//!
//! qalc writes a small, regular subset: \frac, \sqrt[n]{}, ^{} and _{}, \left( \right),
//! \text{}, Greek letters and operators, siunitx's \num{} and \qty{}{} for numbers and units,
//! and matrix environments. Anything else comes through as its name, upright.
//!
//! The tree, as JSON:
//!   "text"            upright text (numbers, operators with their spacing, units, names)
//!   {"v": "x"}        a variable, italic
//!   [a, b, ...]       a row
//!   {"frac": [a, b]}  {"sqrt": a, "n": b}  {"sup": [a, b]}  {"sub": [a, b]}
//!   {"delim": ["(", a, ")"]}               {"mat": [[a, ...], ...], "l": "[", "r": "]"}

use serde_json::{Value, json};

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    Cmd(String),
    Open,
    Close,
    Sup,
    Sub,
    Amp,
    NewRow,
    Char(char),
}

fn lex(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        out.push(match c {
            '\\' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if i > start {
                    Tok::Cmd(chars[start..i].iter().collect())
                } else if i < chars.len() {
                    i += 1;
                    match chars[i - 1] {
                        '\\' => Tok::NewRow,
                        other => Tok::Cmd(other.to_string()),
                    }
                } else {
                    continue;
                }
            }
            '{' => Tok::Open,
            '}' => Tok::Close,
            '^' => Tok::Sup,
            '_' => Tok::Sub,
            '&' => Tok::Amp,
            c => Tok::Char(c),
        });
    }
    out
}

#[derive(Clone, Debug)]
enum Node {
    Text(String),
    /// A run of digits (and a decimal point), grouped when written out.
    Num(String),
    /// +, =, ×, ...: spaced when they sit between two things.
    Op(String),
    Var(String),
    Row(Vec<Node>),
    Frac(Box<Node>, Box<Node>),
    Sqrt(Box<Node>, Option<Box<Node>>),
    Sup(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Delim(String, Box<Node>, String),
    Mat(Vec<Vec<Node>>, String, String),
}

struct Parser {
    toks: Vec<Tok>,
    at: usize,
}

const GREEK: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ϵ"),
    ("varepsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("vartheta", "ϑ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "ϕ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
];

const OPS: &[(&str, &str)] = &[
    ("cdot", "·"),
    ("times", "×"),
    ("div", "÷"),
    ("pm", "±"),
    ("mp", "∓"),
    ("approx", "≈"),
    ("neq", "≠"),
    ("ne", "≠"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("lt", "<"),
    ("gt", ">"),
    ("to", "→"),
    ("rightarrow", "→"),
    ("equiv", "≡"),
    ("in", "∈"),
];

const SYMBOLS: &[(&str, &str)] = &[
    ("infty", "∞"),
    ("degree", "°"),
    ("circ", "°"),
    ("ldots", "…"),
    ("dots", "…"),
    ("cdots", "⋯"),
    ("angle", "∠"),
    ("hbar", "ℏ"),
    ("ell", "ℓ"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("emptyset", "∅"),
    ("%", "%"),
    ("{", "{"),
    ("}", "}"),
    ("$", "$"),
    ("#", "#"),
    ("&", "&"),
    ("_", "_"),
    (",", "\u{2009}"),
    (":", "\u{2005}"),
    (";", "\u{2004}"),
    (" ", " "),
    ("quad", "\u{2003}"),
    ("qquad", "\u{2003}\u{2003}"),
];

const FUNCTIONS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh",
    "coth", "ln", "log", "lg", "exp", "det", "dim", "min", "max", "gcd", "lim", "sup", "inf",
    "arg", "deg", "ker", "Pr",
];

/// siunitx unit macros: prefixes, units, and powers.
const UNITS: &[(&str, &str)] = &[
    ("quecto", "q"),
    ("ronto", "r"),
    ("yocto", "y"),
    ("zepto", "z"),
    ("atto", "a"),
    ("femto", "f"),
    ("pico", "p"),
    ("nano", "n"),
    ("micro", "µ"),
    ("milli", "m"),
    ("centi", "c"),
    ("deci", "d"),
    ("deca", "da"),
    ("deka", "da"),
    ("hecto", "h"),
    ("kilo", "k"),
    ("mega", "M"),
    ("giga", "G"),
    ("tera", "T"),
    ("peta", "P"),
    ("exa", "E"),
    ("zetta", "Z"),
    ("yotta", "Y"),
    ("kibi", "Ki"),
    ("mebi", "Mi"),
    ("gibi", "Gi"),
    ("tebi", "Ti"),
    ("pebi", "Pi"),
    ("m", "m"),
    ("metre", "m"),
    ("meter", "m"),
    ("s", "s"),
    ("second", "s"),
    ("g", "g"),
    ("gram", "g"),
    ("kg", "kg"),
    ("kilogram", "kg"),
    ("minute", "min"),
    ("hour", "h"),
    ("day", "d"),
    ("L", "L"),
    ("l", "L"),
    ("litre", "L"),
    ("liter", "L"),
    ("K", "K"),
    ("kelvin", "K"),
    ("A", "A"),
    ("ampere", "A"),
    ("V", "V"),
    ("volt", "V"),
    ("W", "W"),
    ("watt", "W"),
    ("J", "J"),
    ("joule", "J"),
    ("N", "N"),
    ("newton", "N"),
    ("Pa", "Pa"),
    ("pascal", "Pa"),
    ("Hz", "Hz"),
    ("hertz", "Hz"),
    ("ohm", "Ω"),
    ("C", "C"),
    ("coulomb", "C"),
    ("F", "F"),
    ("farad", "F"),
    ("T", "T"),
    ("tesla", "T"),
    ("mol", "mol"),
    ("mole", "mol"),
    ("cd", "cd"),
    ("candela", "cd"),
    ("rad", "rad"),
    ("radian", "rad"),
    ("sr", "sr"),
    ("B", "B"),
    ("byte", "B"),
    ("bit", "bit"),
    ("degreeCelsius", "°C"),
    ("celsius", "°C"),
    ("degree", "°"),
    ("arcminute", "′"),
    ("arcsecond", "″"),
    ("percent", "%"),
    ("bar", "bar"),
    ("tonne", "t"),
    ("hectare", "ha"),
    ("electronvolt", "eV"),
    ("eV", "eV"),
    ("au", "au"),
    ("bel", "B"),
    ("decibel", "dB"),
    ("dB", "dB"),
    ("neper", "Np"),
    ("per", "/"),
    ("squared", "²"),
    ("cubed", "³"),
];

fn superscript(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            '-' | '−' => '⁻',
            c => c,
        })
        .collect()
}

fn lookup<'a>(table: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.at).cloned();
        self.at += 1;
        t
    }

    /// Items until a `}`, `&`, `\\`, `\right` or `\end` (left for the caller), or the end.
    fn row(&mut self) -> Option<Vec<Node>> {
        let mut row = Vec::new();
        loop {
            match self.peek() {
                None | Some(Tok::Close | Tok::Amp | Tok::NewRow) => return Some(row),
                Some(Tok::Cmd(c)) if c == "right" || c == "end" => return Some(row),
                _ => self.item(&mut row)?,
            }
        }
    }

    /// `{...}` as a row, or a single item.
    fn arg(&mut self) -> Option<Node> {
        while matches!(self.peek(), Some(Tok::Char(c)) if c.is_whitespace()) {
            self.at += 1;
        }
        if self.peek() == Some(&Tok::Open) {
            self.at += 1;
            let row = self.row()?;
            (self.next()? == Tok::Close).then_some(Node::Row(row))
        } else {
            let mut row = Vec::new();
            self.item(&mut row)?;
            Some(Node::Row(row))
        }
    }

    /// `[...]` if there is one, as raw text.
    fn optional(&mut self) -> Option<String> {
        if self.peek() != Some(&Tok::Char('[')) {
            return None;
        }
        self.at += 1;
        let mut raw = String::new();
        while let Some(t) = self.next() {
            match t {
                Tok::Char(']') => break,
                Tok::Char(c) => raw.push(c),
                Tok::Cmd(c) => raw.push_str(&c),
                _ => {}
            }
        }
        Some(raw)
    }

    /// `{...}` as raw text, commands mapped through `map` (or kept as their names).
    fn raw(&mut self, map: &dyn Fn(&str, &mut Self) -> String) -> Option<String> {
        if self.next()? != Tok::Open {
            return None;
        }
        let mut out = String::new();
        let mut depth = 0;
        loop {
            match self.next()? {
                Tok::Open => depth += 1,
                Tok::Close if depth == 0 => return Some(out),
                Tok::Close => depth -= 1,
                Tok::Char(c) => out.push(c),
                Tok::Cmd(c) => {
                    let mapped = map(&c, self);
                    out.push_str(&mapped);
                }
                Tok::Sup => out.push('^'),
                Tok::Sub => out.push('_'),
                Tok::Amp => out.push('&'),
                Tok::NewRow => out.push(' '),
            }
        }
    }

    fn text_cmd(name: &str, _: &mut Self) -> String {
        lookup(SYMBOLS, name)
            .or(lookup(GREEK, name))
            .map_or_else(|| name.to_string(), String::from)
    }

    fn unit_cmd(name: &str, p: &mut Self) -> String {
        if name == "tothe" {
            return p
                .raw(&Self::text_cmd)
                .map(|e| superscript(&e))
                .unwrap_or_default();
        }
        if name == "raiseto" {
            // \raiseto{4}\m: the power comes first; good enough to put it after.
            return p
                .raw(&Self::text_cmd)
                .map(|e| superscript(&e))
                .unwrap_or_default();
        }
        lookup(UNITS, name).map_or_else(|| name.to_string(), String::from)
    }

    fn item(&mut self, row: &mut Vec<Node>) -> Option<()> {
        match self.next()? {
            Tok::Open => {
                let inner = self.row()?;
                if self.next()? != Tok::Close {
                    return None;
                }
                row.push(Node::Row(inner));
            }
            Tok::Close | Tok::Amp | Tok::NewRow => return None,
            Tok::Sup | Tok::Sub => {
                let sub = self.toks[self.at - 1] == Tok::Sub;
                let base = row.pop().unwrap_or(Node::Text(String::new()));
                let script = self.arg()?;
                row.push(match sub {
                    true => Node::Sub(Box::new(base), Box::new(script)),
                    false => Node::Sup(Box::new(base), Box::new(script)),
                });
            }
            Tok::Char(c) if c.is_whitespace() => {}
            Tok::Char(c) if c.is_ascii_digit() || c == '.' => match row.last_mut() {
                Some(Node::Num(n)) => n.push(c),
                _ => row.push(Node::Num(c.to_string())),
            },
            Tok::Char(c) if c.is_alphabetic() => row.push(Node::Var(c.to_string())),
            Tok::Char(c @ ('+' | '=' | '<' | '>' | '±' | '×' | '·' | '≈' | '≠' | '≤' | '≥')) => {
                row.push(Node::Op(c.to_string()))
            }
            Tok::Char('-' | '−') => row.push(Node::Op("−".into())),
            Tok::Char('*') => row.push(Node::Op("×".into())),
            Tok::Char(c) => row.push(Node::Text(c.to_string())),
            Tok::Cmd(name) => self.command(&name, row)?,
        }
        Some(())
    }

    fn command(&mut self, name: &str, row: &mut Vec<Node>) -> Option<()> {
        match name {
            "displaystyle" | "textstyle" | "scriptstyle" | "limits" | "nolimits" | "!" => {}
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let a = self.arg()?;
                let b = self.arg()?;
                row.push(Node::Frac(Box::new(a), Box::new(b)));
            }
            "sqrt" => {
                let index = self.optional().map(|n| Box::new(Node::Num(n)));
                let a = self.arg()?;
                row.push(Node::Sqrt(Box::new(a), index));
            }
            "left" => {
                let open = self.delimiter()?;
                let inner = self.row()?;
                if self.next()? != Tok::Cmd("right".into()) {
                    return None;
                }
                let close = self.delimiter()?;
                row.push(Node::Delim(open, Box::new(Node::Row(inner)), close));
            }
            "text" | "textrm" | "mathrm" | "textup" | "operatorname" | "mbox" | "textnormal" => {
                let t = self.raw(&Self::text_cmd)?;
                row.push(Node::Text(t));
            }
            "mathit" | "textit" | "mathbf" | "textbf" | "boldsymbol" | "mathsf" | "mathtt" => {
                let a = self.arg()?;
                row.push(a);
            }
            "num" => {
                self.optional();
                let n = self.raw(&Self::text_cmd)?;
                row.push(number(&n));
            }
            "qty" | "SI" => {
                self.optional();
                let value = self.arg()?;
                // Spaces only separate macros in a unit (\mega B).
                let unit: String = (self.raw(&Self::unit_cmd)?.chars())
                    .filter(|c| !c.is_whitespace())
                    .collect();
                let unit = match unit.as_str() {
                    // qalc's names for these, which siunitx would spell as macros.
                    "oC" => "°C".into(),
                    "oF" => "°F".into(),
                    _ => unit,
                };
                row.push(Node::Row(vec![
                    value,
                    Node::Text(format!("\u{2009}{unit}")),
                ]));
            }
            "unit" | "si" => {
                self.optional();
                // Spaces only separate macros in a unit (\mega B).
                let unit: String = (self.raw(&Self::unit_cmd)?.chars())
                    .filter(|c| !c.is_whitespace())
                    .collect();
                row.push(Node::Text(unit));
            }
            "begin" => {
                let env = self.raw(&Self::text_cmd)?;
                let (l, r) = match env.as_str() {
                    "bmatrix" => ("[", "]"),
                    "pmatrix" => ("(", ")"),
                    "vmatrix" => ("|", "|"),
                    "Vmatrix" => ("‖", "‖"),
                    "Bmatrix" => ("{", "}"),
                    "matrix" | "smallmatrix" => ("", ""),
                    _ => return None,
                };
                let mut rows = vec![Vec::new()];
                loop {
                    let cell = self.row()?;
                    rows.last_mut().unwrap().push(Node::Row(cell));
                    match self.next()? {
                        Tok::Amp => {}
                        Tok::NewRow => rows.push(Vec::new()),
                        Tok::Cmd(c) if c == "end" => {
                            self.raw(&Self::text_cmd)?;
                            break;
                        }
                        _ => return None,
                    }
                }
                rows.retain(|r| !r.is_empty());
                row.push(Node::Mat(rows, l.into(), r.into()));
            }
            name if FUNCTIONS.contains(&name) => row.push(Node::Text(name.into())),
            name => {
                if let Some(op) = lookup(OPS, name) {
                    row.push(Node::Op(op.into()));
                } else if let Some(g) = lookup(GREEK, name) {
                    row.push(Node::Text(g.into()));
                } else {
                    let s = lookup(SYMBOLS, name).unwrap_or(name);
                    row.push(Node::Text(s.into()));
                }
            }
        }
        Some(())
    }

    fn delimiter(&mut self) -> Option<String> {
        Some(match self.next()? {
            Tok::Char('.') => String::new(),
            Tok::Char(c) => c.to_string(),
            Tok::Cmd(c) => match c.as_str() {
                "{" | "lbrace" => "{".into(),
                "}" | "rbrace" => "}".into(),
                "|" | "Vert" => "‖".into(),
                "langle" => "⟨".into(),
                "rangle" => "⟩".into(),
                "lfloor" => "⌊".into(),
                "rfloor" => "⌋".into(),
                "lceil" => "⌈".into(),
                "rceil" => "⌉".into(),
                "vert" => "|".into(),
                _ => return None,
            },
            _ => return None,
        })
    }
}

/// 1 222 333.5: thin spaces between groups of three, once there are more than four digits.
pub fn group(digits: &str) -> String {
    const THIN: char = '\u{2009}';
    let (int, frac) = digits
        .split_once('.')
        .map_or((digits, None), |(i, f)| (i, Some(f)));
    let mut out = String::new();
    if int.len() > 4 {
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                out.push(THIN);
            }
            out.push(c);
        }
    } else {
        out.push_str(int);
    }
    if let Some(frac) = frac {
        out.push('.');
        for (i, c) in frac.chars().enumerate() {
            if frac.len() > 4 && i > 0 && i % 3 == 0 {
                out.push(THIN);
            }
            out.push(c);
        }
    }
    out
}

/// \num's contents: digits, maybe a sign and an exponent (2.65e32 → 2.65 × 10³²).
fn number(raw: &str) -> Node {
    let raw = raw.trim();
    let (mantissa, exp) = match raw.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e.trim_start_matches('+'))),
        None => (raw, None),
    };
    let (sign, mantissa) = match mantissa.strip_prefix(['-', '−']) {
        Some(m) => (Some(Node::Op("−".into())), m),
        None => (None, mantissa),
    };
    let mut row: Vec<Node> = sign.into_iter().collect();
    row.push(Node::Num(mantissa.into()));
    if let Some(e) = exp {
        row.push(Node::Op("×".into()));
        let e = e.replace('-', "−");
        row.push(Node::Sup(
            Box::new(Node::Num("10".into())),
            Box::new(Node::Text(e)),
        ));
    }
    Node::Row(row)
}

/// Operators get spaces when they're between two things (2 + 3), none when unary (−3).
fn to_json(node: &Node) -> Value {
    match node {
        Node::Text(t) => json!(t),
        Node::Num(n) => json!(group(n)),
        Node::Op(o) => json!(o),
        Node::Var(v) => json!({ "v": v }),
        Node::Row(items) => {
            let mut out: Vec<Value> = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let v = match item {
                    Node::Op(o) => {
                        let binary = i > 0 && !matches!(items[i - 1], Node::Op(_));
                        json!(if binary {
                            format!("\u{2005}{o}\u{2005}")
                        } else {
                            o.clone()
                        })
                    }
                    other => to_json(other),
                };
                // Rows inside rows flatten, and neighbouring text joins up.
                let pieces = match v {
                    Value::Array(inner) if !matches!(item, Node::Mat(..)) => inner,
                    v => vec![v],
                };
                for p in pieces {
                    match (out.last_mut(), &p) {
                        (Some(Value::String(prev)), Value::String(s)) => prev.push_str(s),
                        _ => out.push(p),
                    }
                }
            }
            match out.len() {
                1 => out.pop().unwrap(),
                _ => Value::Array(out),
            }
        }
        Node::Frac(a, b) => json!({ "frac": [to_json(a), to_json(b)] }),
        Node::Sqrt(a, None) => json!({ "sqrt": to_json(a) }),
        Node::Sqrt(a, Some(n)) => json!({ "sqrt": to_json(a), "n": to_json(n) }),
        Node::Sup(a, b) => json!({ "sup": [to_json(a), to_json(b)] }),
        Node::Sub(a, b) => json!({ "sub": [to_json(a), to_json(b)] }),
        Node::Delim(l, a, r) => json!({ "delim": [l, to_json(a), r] }),
        Node::Mat(rows, l, r) => json!({
            "mat": rows.iter().map(|r| r.iter().map(to_json).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "l": l,
            "r": r,
        }),
    }
}

/// The LaTeX as qalc prints it (with its `$`s), as a tree; None for anything unexpected.
pub fn parse(latex: &str) -> Option<Value> {
    let src = latex.trim().trim_start_matches('$').trim_end_matches('$');
    let mut p = Parser {
        toks: lex(src),
        at: 0,
    };
    let row = p.row()?;
    (p.at >= p.toks.len()).then(|| to_json(&Node::Row(row)))
}

/// The LaTeX to copy: without qalc's `$\displaystyle ...$` wrapping.
pub fn clean(latex: &str) -> String {
    let s = latex
        .trim()
        .trim_start_matches('$')
        .trim_end_matches('$')
        .trim();
    s.strip_prefix("\\displaystyle")
        .unwrap_or(s)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(s: &str) -> String {
        // Thin and medium spaces as plain ones, to keep the expectations readable.
        parse(s)
            .map(|v| v.to_string().replace(['\u{2009}', '\u{2005}'], " "))
            .unwrap_or_else(|| "FAILED".into())
    }

    #[test]
    fn fractions_and_symbols() {
        assert_eq!(
            tree(r"$\displaystyle x = 2\pi n + \frac{\pi }{2}$"),
            r#"[{"v":"x"}," = 2π",{"v":"n"}," + ",{"frac":["π","2"]}]"#
        );
        assert_eq!(
            tree(r"$\displaystyle x = -\sqrt{2}\text{ or }x = \sqrt{2}$"),
            r#"[{"v":"x"}," = −",{"sqrt":"2"}," or ",{"v":"x"}," = ",{"sqrt":"2"}]"#
        );
        assert_eq!(
            tree(r"$x^{2} + 2x + 1$"),
            r#"[{"sup":[{"v":"x"},"2"]}," + 2",{"v":"x"}," + 1"]"#
        );
        assert_eq!(
            tree(r"$\displaystyle \sqrt[3]{5}$"),
            r#"{"n":"3","sqrt":"5"}"#
        );
        assert_eq!(
            tree(r"$\displaystyle e^{\left(2x\right)}$"),
            r#"{"sup":[{"v":"e"},{"delim":["(",["2",{"v":"x"}],")"]}]}"#
        );
        assert_eq!(tree(r"$\displaystyle \ln(2)$"), r#""ln(2)""#);
    }

    #[test]
    fn numbers_and_units() {
        assert_eq!(
            tree(r"$\displaystyle \num[parse-numbers=true]{1222333}$"),
            r#""1 222 333""#
        );
        assert_eq!(
            tree(r"$\displaystyle \num{2.652528598e32}$"),
            r#"["2.652 528 598 × ",{"sup":["10","32"]}]"#
        );
        assert_eq!(
            tree(r"$\displaystyle \qty[parse-numbers=false]{\frac{25}{18}}{\m\per\second}$"),
            r#"[{"frac":["25","18"]}," m/s"]"#
        );
        assert_eq!(tree(r"$\displaystyle \qty{3}{\m\squared}$"), r#""3 m²""#);
        assert_eq!(
            tree(r"$\displaystyle \qty{1073.741824}{\mega B}$"),
            r#""1073.741 824 MB""#
        );
        assert_eq!(tree(r"$\displaystyle \qty{68}{oF}$"), r#""68 °F""#);
        assert_eq!(
            tree(r"$\displaystyle \text{2026-10-21}$"),
            r#""2026-10-21""#
        );
    }

    #[test]
    fn matrices() {
        assert_eq!(
            tree(r"$\displaystyle \begin{bmatrix}1 & 2 \\ 3 & 4\end{bmatrix}$"),
            r#"{"l":"[","mat":[["1","2"],["3","4"]],"r":"]"}"#
        );
    }

    #[test]
    fn grouping() {
        assert_eq!(group("1222333"), "1\u{2009}222\u{2009}333");
        assert_eq!(group("1073"), "1073");
        assert_eq!(group("12345.678901"), "12\u{2009}345.678\u{2009}901");
        assert_eq!(clean(r"$\displaystyle \frac{1}{2}$"), r"\frac{1}{2}");
    }

    #[test]
    fn rejects() {
        assert_eq!(tree(r"$\frac{1}$"), "FAILED");
        assert_eq!(tree(r"$\begin{weird}x\end{weird}$"), "FAILED");
    }
}
