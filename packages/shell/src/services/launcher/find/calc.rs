//! The launcher's `=` calculator (qs-find's calc.rs): Qalculate's `qalc`, so it answers what the
//! Qalculate app does: units and currencies (`5 km/h to m/s`, `10 USD to SEK`), dates
//! (`today + 30 days`), equations (`x^2 = 2`, `solve(...)`), and plain arithmetic. qalc has to
//! be on the shell's PATH.
//!
//! Three runs per expression, side by side (about 25 ms): the exact result, which keeps
//! symbols and fractions (`x = 2πn + π / 2`), once as text and once as LaTeX (typeset by the
//! launcher, see tex.rs), and the decimal value, for copying. Typed decimals stay decimals
//! (`1.5`, not `1 + 1/2`). Numbers read 1 222 333.5. All stop when a newer expression comes in
//! (the launcher aborts the task) or after a few seconds (`100000!` takes a while).

use std::process::{Output, Stdio};
use std::time::Duration;

use serde_json::Value;
use tokio::process::{Child, Command};

const TIMEOUT: Duration = Duration::from_secs(3);
const FORMAT: [&str; 4] = ["-s", "decimal comma 0", "-s", "digit grouping 1"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Answer {
    pub ok: bool,
    /// The exact form, or why there is no answer.
    pub text: String,
    /// The exact form typeset (tex.rs), when qalc's LaTeX could be read.
    pub math: Option<Value>,
    /// The decimal value, when it reads differently from `text`.
    pub approx: String,
    /// What to copy: the decimal value (whether or not it's shown), the exact form, LaTeX.
    pub value: String,
    pub exact: String,
    pub tex: String,
}

fn spawn(settings: &[&str], expr: &str) -> Option<Child> {
    Command::new("qalc")
        .arg("-t")
        .args(FORMAT)
        .args(settings)
        .args(["--", expr])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Killed when a newer expression drops this one, or it takes too long.
        .kill_on_drop(true)
        .spawn()
        .ok()
}

/// The run's one-line output, or None if it failed.
fn output(run: std::io::Result<Output>) -> Option<String> {
    let run = run.ok()?;
    let out = String::from_utf8_lossy(&run.stdout);
    let text = (out.lines())
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("  ");
    (run.status.success() && !text.is_empty()).then_some(text)
}

fn fail(text: &str) -> Answer {
    Answer {
        ok: false,
        text: text.into(),
        math: None,
        approx: String::new(),
        value: String::new(),
        exact: String::new(),
        tex: String::new(),
    }
}

/// For copying: no grouping between digits (1222333.5), plain spaces elsewhere (1.5 m/s).
fn plain(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if matches!(c, '\u{2009}' | '\u{202f}' | '\u{a0}') {
            let digit = |j: Option<&char>| j.is_some_and(char::is_ascii_digit);
            if !(i > 0 && digit(chars.get(i - 1)) && digit(chars.get(i + 1))) {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A number typed with a decimal point, like 1.5 or .25.
fn has_decimal(expr: &str) -> bool {
    let b = expr.as_bytes();
    (0..b.len()).any(|i| b[i] == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
}

/// Runs qalc on `expr`. Dropping the future (aborting its task) stops the runs.
pub async fn eval(expr: &str) -> Answer {
    let expr = expr.trim();
    if expr.is_empty() {
        return fail("Type an expression");
    }
    // "auto" writes 1.5 as 1 + 1/2; "exact" only uses fractions where decimals can't be exact.
    let fractions = if has_decimal(expr) {
        "fractions exact"
    } else {
        "fractions auto"
    };
    let exact_settings = ["-s", "approximation exact", "-s", fractions];
    let latex_settings = ["-latex", "-s", "approximation exact", "-s", fractions];
    let mut runs = Vec::new();
    for settings in [&exact_settings[..], &latex_settings[..], &[]] {
        match spawn(settings, expr) {
            Some(c) => runs.push(c.wait_with_output()),
            None => return fail("qalc isn't installed"),
        }
    }
    let Ok(outs) = tokio::time::timeout(TIMEOUT, futures::future::join_all(runs)).await else {
        return fail("Takes too long");
    };
    let mut outs = outs.into_iter().map(output);
    let (exact, latex, approx) = (
        outs.next().flatten(),
        outs.next().flatten(),
        outs.next().flatten(),
    );
    answer(exact, latex, approx)
}

/// The three runs' outputs (each None if it failed) as an answer.
fn answer(exact: Option<String>, latex: Option<String>, approx: Option<String>) -> Answer {
    let (text, approx) = match (exact, approx) {
        (Some(e), Some(a)) => (e, a),
        (Some(t), None) | (None, Some(t)) => (t.clone(), t),
        (None, None) => return fail("Not an expression"),
    };
    // "1/2 = 0.5" already shows the value.
    let shown = if approx == text || text.ends_with(&format!("= {approx}")) {
        String::new()
    } else {
        approx.clone()
    };
    Answer {
        ok: true,
        // A tree that's only text adds nothing over qalc's text, which rounds the way qalc
        // shows numbers (LaTeX gets every digit: 99.17967008 SEK for 99.1797 SEK).
        math: (latex.as_deref().and_then(super::tex::parse)).filter(|m| !m.is_string()),
        tex: latex.as_deref().map(super::tex::clean).unwrap_or_default(),
        value: plain(&approx),
        exact: plain(&text),
        approx: shown,
        text,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn decimals() {
        assert!(super::has_decimal("1.5 + 2"));
        assert!(super::has_decimal(".25"));
        assert!(!super::has_decimal("1/2"));
        assert!(!super::has_decimal("5 km/h to m/s"));
    }

    #[test]
    fn copying() {
        assert_eq!(super::plain("1\u{202f}222\u{202f}333.5"), "1222333.5");
        assert_eq!(super::plain("1.388\u{2009}888\u{202f}m/s"), "1.388888 m/s");
    }

    #[test]
    fn answers() {
        let some = |s: &str| Some(s.to_string());
        let a = super::answer(
            some("(25/18) m/s"),
            some(r"$\displaystyle \qty[parse-numbers=false]{\frac{25}{18}}{\m\per\second}$"),
            some("1.388\u{2009}888\u{2009}889\u{202f}m/s"),
        );
        assert!(a.ok);
        assert_eq!(a.approx, "1.388\u{2009}888\u{2009}889\u{202f}m/s");
        assert_eq!(a.value, "1.388888889 m/s");
        assert_eq!(a.exact, "(25/18) m/s");
        assert_eq!(
            a.tex,
            r"\qty[parse-numbers=false]{\frac{25}{18}}{\m\per\second}"
        );
        assert!(a.math.is_some_and(|m| m.is_array()));

        // The value already shown, and a tree that's only text.
        let a = super::answer(some("1/2 = 0.5"), some(r"$\text{x}$"), some("0.5"));
        assert_eq!((a.approx.as_str(), a.math), ("", None));
        let a = super::answer(some("3"), some(r"$\displaystyle 3$"), some("3"));
        assert_eq!((a.approx.as_str(), a.value.as_str()), ("", "3"));

        // One run failing still answers; all failing doesn't.
        let a = super::answer(None, None, some("2026-11-05"));
        assert_eq!(
            (a.ok, a.text.as_str(), a.exact.as_str()),
            (true, "2026-11-05", "2026-11-05")
        );
        let a = super::answer(None, some("$x$"), None);
        assert_eq!((a.ok, a.text.as_str()), (false, "Not an expression"));
    }

    #[test]
    fn empty() {
        let a = futures::executor::block_on(super::eval("  "));
        assert_eq!((a.ok, a.text.as_str()), (false, "Type an expression"));
    }
}
