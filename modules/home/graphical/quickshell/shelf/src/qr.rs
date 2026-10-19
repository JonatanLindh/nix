//! QR codes as SVG path data, so the shell draws them as a crisp vector Shape at any size.

use qrcode::{Color, QrCode};
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct Qr {
    /// Modules per side (no quiet zone; the UI pads it).
    pub size: usize,
    /// Dark modules as `M x y h n v 1 h -n z` runs, in module units.
    pub path: String,
}

pub fn encode(data: &str) -> anyhow::Result<Qr> {
    let code = QrCode::new(data.as_bytes())?;
    let size = code.width();
    let colors = code.to_colors();
    let mut path = String::new();
    for (y, row) in colors.chunks(size).enumerate() {
        let mut x = 0;
        while x < size {
            if row[x] == Color::Dark {
                let run = row[x..].iter().take_while(|&&c| c == Color::Dark).count();
                path.push_str(&format!("M{x} {y}h{run}v1h-{run}z"));
                x += run;
            } else {
                x += 1;
            }
        }
    }
    Ok(Qr { size, path })
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_runs() {
        let qr = super::encode("7-crossover-clockwork").unwrap();
        assert_eq!(qr.size, 25);
        // The top-left finder pattern starts with a run of seven.
        assert!(qr.path.starts_with("M0 0h7v1h-7z"));
    }
}
