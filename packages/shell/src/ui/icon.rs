//! Icons from `assets/icons.json` (the path data from Quickshell's Icons.qml): 24x24 SVG paths in
//! four layers, `stroke` and `fill` plus "faint" variants drawn at reduced opacity.
//!
//! Each icon is built once as a white SVG with the faint layers at their opacity, and served to
//! gpui as the asset `icons/<name>.svg`. gpui draws an SVG as a mask in the element's text
//! colour, keeping the mask's alpha, so the faint parts survive the tint.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::LazyLock;

use gpui::{AssetSource, Rgba, SharedString, Svg, prelude::*, px, svg};
use serde::Deserialize;

/// Stroke width in 24-unit icon space, like the SVG stroke-width on the design canvas.
const STROKE_WIDTH: f32 = 1.9;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Paths {
    #[serde(default)]
    stroke: String,
    #[serde(default)]
    fill: String,
    #[serde(default)]
    stroke_faint: String,
    #[serde(default)]
    fill_faint: String,
}

impl Paths {
    fn to_svg(&self) -> String {
        let mut layers = String::new();
        let mut layer = |d: &str, attrs: &str| {
            if !d.is_empty() {
                layers.push_str(&format!(r#"<path d="{d}" {attrs}/>"#));
            }
        };
        let stroke = format!(
            r#"fill="none" stroke="white" stroke-width="{STROKE_WIDTH}" stroke-linecap="round" stroke-linejoin="round""#
        );
        // Same order as Icon.qml: faint fill, faint stroke, fill, stroke.
        layer(&self.fill_faint, r#"fill="white" fill-opacity="0.45""#);
        layer(
            &self.stroke_faint,
            &format!(r#"{stroke} stroke-opacity="0.3""#),
        );
        layer(&self.fill, r#"fill="white""#);
        layer(&self.stroke, &stroke);
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">{layers}</svg>"#)
    }
}

static ICONS: LazyLock<HashMap<String, Vec<u8>>> = LazyLock::new(|| {
    let paths: HashMap<String, Paths> =
        serde_json::from_str(include_str!("../../assets/icons.json")).expect("icons.json");
    paths
        .into_iter()
        .map(|(name, p)| (format!("icons/{name}.svg"), p.to_svg().into_bytes()))
        // The battery glyph's bolt, on its 28x13 canvas.
        .chain([(
            "icons/battery-bolt.svg".to_string(),
            br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 28 13"><path d="M13.2 1.5 8.6 7.3h3.4l-1 4.2 4.8-6.1h-3.4z" fill="white"/></svg>"#.to_vec(),
        )])
        .collect()
});

/// The app's assets: the icons, plus anything extra drawn at runtime (see [`Assets::svg`]).
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS.get(path).map(|svg| Cow::Owned(svg.clone())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICONS
            .keys()
            .filter(|k| k.starts_with(path))
            .map(|k| k.clone().into())
            .collect())
    }
}

/// The asset path of icon `name`.
pub fn path(name: &str) -> SharedString {
    debug_assert!(
        ICONS.contains_key(&format!("icons/{name}.svg")),
        "unknown icon {name:?}"
    );
    format!("icons/{name}.svg").into()
}

/// The icon `name` at `size` pixels in `color` (its alpha included).
pub fn icon(name: &str, size: f32, color: Rgba) -> Svg {
    svg()
        .path(path(name))
        .size(px(size))
        .flex_none()
        .text_color(color)
}
