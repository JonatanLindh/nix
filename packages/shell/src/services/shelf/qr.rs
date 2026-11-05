//! QR codes as runs of dark modules, which the shelf draws as rectangles at any size.

use qrcode::{Color, QrCode};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Qr {
    /// Modules per side (no quiet zone; the UI pads it).
    pub size: usize,
    runs: Vec<Run>,
}

/// `len` dark modules from column `x` of row `y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub x: usize,
    pub y: usize,
    pub len: usize,
}

impl Qr {
    /// The dark modules, row by row.
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }
}

pub fn encode(data: &str) -> anyhow::Result<Qr> {
    let code = QrCode::new(data.as_bytes())?;
    let size = code.width();
    let colors = code.to_colors();
    let mut runs = Vec::new();
    for (y, row) in colors.chunks(size).enumerate() {
        let mut x = 0;
        while x < size {
            if row[x] == Color::Dark {
                let len = row[x..].iter().take_while(|&&c| c == Color::Dark).count();
                runs.push(Run { x, y, len });
                x += len;
            } else {
                x += 1;
            }
        }
    }
    Ok(Qr { size, runs })
}

#[cfg(test)]
mod tests {
    use super::Run;

    #[test]
    fn encodes_runs() {
        let qr = super::encode("7-crossover-clockwork").unwrap();
        assert_eq!(qr.size, 25);
        // The top-left finder pattern starts with a run of seven.
        assert_eq!(qr.runs()[0], Run { x: 0, y: 0, len: 7 });
        assert!(
            qr.runs()
                .iter()
                .all(|r| r.x + r.len <= qr.size && r.y < qr.size)
        );
    }
}
