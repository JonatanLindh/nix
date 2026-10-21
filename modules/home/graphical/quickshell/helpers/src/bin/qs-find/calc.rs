//! The launcher's `=` calculator: + - * / % ^ (or **), parentheses, factorial (!), the
//! constants pi, tau and e, and functions like sqrt(2) or sin(pi/2) (radians). A number
//! straight before a name or a parenthesis multiplies: 2pi, 3(4 + 1).

type Function = fn(f64) -> f64;

const FUNCTIONS: &[(&str, Function)] = &[
    ("sqrt", f64::sqrt),
    ("cbrt", f64::cbrt),
    ("abs", f64::abs),
    ("exp", f64::exp),
    ("ln", f64::ln),
    ("log", f64::log10),
    ("log10", f64::log10),
    ("log2", f64::log2),
    ("sin", f64::sin),
    ("cos", f64::cos),
    ("tan", f64::tan),
    ("asin", f64::asin),
    ("acos", f64::acos),
    ("atan", f64::atan),
    ("sinh", f64::sinh),
    ("cosh", f64::cosh),
    ("tanh", f64::tanh),
    ("floor", f64::floor),
    ("ceil", f64::ceil),
    ("round", f64::round),
];

const CONSTANTS: &[(&str, f64)] = &[
    ("pi", std::f64::consts::PI),
    ("π", std::f64::consts::PI),
    ("tau", std::f64::consts::TAU),
    ("e", std::f64::consts::E),
];

#[derive(Clone, PartialEq, Debug)]
enum Token {
    Num(f64),
    Name(String),
    Op(char),
}

fn tokens(src: &str) -> Option<Vec<Token>> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            // 1e3, 2.5E-4; but "2e" alone is 2 times e.
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let mut j = i + 1;
                if j < chars.len() && (chars[j] == '+' || chars[j] == '-') {
                    j += 1;
                }
                if j < chars.len() && chars[j].is_ascii_digit() {
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            out.push(Token::Num(
                chars[start..i].iter().collect::<String>().parse().ok()?,
            ));
        } else if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric()) {
                i += 1;
            }
            out.push(Token::Name(
                chars[start..i].iter().collect::<String>().to_lowercase(),
            ));
        } else {
            let op = match c {
                '×' | '·' => '*',
                '÷' => '/',
                '−' => '-',
                '*' if chars.get(i + 1) == Some(&'*') => {
                    i += 1;
                    '^'
                }
                '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')' | '!' => c,
                _ => return None,
            };
            out.push(Token::Op(op));
            i += 1;
        }
    }
    Some(out)
}

struct Parser {
    tokens: Vec<Token>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Token::Op(op)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    // expr := term (('+' | '-') term)*
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Some(v);
            }
        }
    }

    // term := unary (('*' | '/' | '%' | nothing) unary)*
    fn term(&mut self) -> Option<f64> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') {
                v *= self.unary()?;
            } else if self.eat('/') {
                v /= self.unary()?;
            } else if self.eat('%') {
                v %= self.unary()?;
            } else if matches!(
                self.peek(),
                Some(Token::Num(_) | Token::Name(_) | Token::Op('('))
            ) {
                v *= self.unary()?;
            } else {
                return Some(v);
            }
        }
    }

    // unary := ('-' | '+') unary | power; so -2^2 is -4, and 2^-1 works.
    fn unary(&mut self) -> Option<f64> {
        if self.eat('-') {
            return Some(-self.unary()?);
        }
        if self.eat('+') {
            return self.unary();
        }
        self.power()
    }

    // power := postfix ('^' unary)?, right-associative.
    fn power(&mut self) -> Option<f64> {
        let base = self.postfix()?;
        if self.eat('^') {
            return Some(base.powf(self.unary()?));
        }
        Some(base)
    }

    // postfix := primary '!'*
    fn postfix(&mut self) -> Option<f64> {
        let mut v = self.primary()?;
        while self.eat('!') {
            if v < 0.0 || v.fract() != 0.0 || v > 170.0 {
                return None;
            }
            v = (1..=v as u64).map(|n| n as f64).product();
        }
        Some(v)
    }

    fn primary(&mut self) -> Option<f64> {
        match self.peek()?.clone() {
            Token::Num(n) => {
                self.at += 1;
                Some(n)
            }
            Token::Op('(') => {
                self.at += 1;
                let v = self.expr()?;
                // A missing ")" at the very end is forgiven, as you'd still be typing it.
                if !self.eat(')') && self.peek().is_some() {
                    return None;
                }
                Some(v)
            }
            Token::Name(name) => {
                self.at += 1;
                if let Some((_, f)) = FUNCTIONS.iter().find(|(n, _)| *n == name) {
                    // sqrt(2), or sqrt 2 like on a calculator.
                    return Some(f(self.power()?));
                }
                CONSTANTS.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
            }
            Token::Op(_) => None,
        }
    }
}

pub fn eval(src: &str) -> Option<f64> {
    let mut p = Parser {
        tokens: tokens(src)?,
        at: 0,
    };
    let v = p.expr()?;
    (p.at == p.tokens.len() && v.is_finite()).then_some(v)
}

/// 12 significant digits, so 0.1 + 0.2 is 0.3; exponent notation when very large or small.
pub fn format(v: f64) -> String {
    let v: f64 = format!("{v:.11e}").parse().unwrap_or(v);
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e15 || v.abs() < 1e-7 {
        return format!("{v:e}");
    }
    format!("{v}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(s: &str) -> Option<String> {
        eval(s).map(format)
    }

    #[test]
    fn arithmetic() {
        assert_eq!(calc("1+2*3").as_deref(), Some("7"));
        assert_eq!(calc("(1+2)*3").as_deref(), Some("9"));
        assert_eq!(calc("2^10").as_deref(), Some("1024"));
        assert_eq!(calc("2**3**2").as_deref(), Some("512"));
        assert_eq!(calc("-2^2").as_deref(), Some("-4"));
        assert_eq!(calc("2^-1").as_deref(), Some("0.5"));
        assert_eq!(calc("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(calc("7 % 3").as_deref(), Some("1"));
        assert_eq!(calc("5!").as_deref(), Some("120"));
        assert_eq!(calc("1e3 + 1").as_deref(), Some("1001"));
        assert_eq!(calc("3 × 4 ÷ 2").as_deref(), Some("6"));
    }

    #[test]
    fn names() {
        assert_eq!(calc("2pi").as_deref(), Some("6.28318530718"));
        assert_eq!(calc("sqrt(16)").as_deref(), Some("4"));
        assert_eq!(calc("sqrt 16").as_deref(), Some("4"));
        assert_eq!(calc("sin(pi/2)").as_deref(), Some("1"));
        assert_eq!(calc("3(4+1)").as_deref(), Some("15"));
        assert_eq!(calc("2e").as_deref(), Some("5.43656365692"));
        assert_eq!(calc("log 1000").as_deref(), Some("3"));
        assert_eq!(calc("(1+2").as_deref(), Some("3"));
    }

    #[test]
    fn rejects() {
        assert_eq!(calc("hello"), None);
        assert_eq!(calc("1/0"), None);
        assert_eq!(calc("1 +"), None);
        assert_eq!(calc("(-1)!"), None);
        assert_eq!(calc("2)"), None);
    }

    #[test]
    fn formats() {
        assert_eq!(format(1e21), "1e21");
        assert_eq!(format(-0.0), "0");
        assert_eq!(format(123456.789), "123456.789");
    }
}
