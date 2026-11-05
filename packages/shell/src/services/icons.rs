//! Finding and loading app icons (tray items, notifications, cover art) as gpui images.
//!
//! Theme icons are looked up in Qt's theme first, as Quickshell did, then GTK's, with
//! `-symbolic` as a fallback. Raster pixels are decoded once into a [`RenderImage`], so a shown
//! icon isn't decoded again on every frame.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::RenderImage;

/// An icon or picture, ready to draw.
#[derive(Clone)]
pub enum Image {
    /// Drawn by gpui from the file; monochrome ones can be tinted (`svg().external_path`).
    Svg(PathBuf),
    Raster(Arc<RenderImage>),
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Image::Svg(path) => write!(f, "Svg({})", path.display()),
            Image::Raster(image) => write!(f, "Raster({:?})", image.size(0)),
        }
    }
}

/// The same picture: the same file, or the same decoded pixels.
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Image::Svg(a), Image::Svg(b)) => a == b,
            (Image::Raster(a), Image::Raster(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// Straight RGBA pixels as an image gpui can draw (it wants BGRA).
pub fn raster(width: u32, height: u32, mut rgba: Vec<u8>) -> Option<Arc<RenderImage>> {
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let buffer = ::image::RgbaImage::from_raw(width, height, rgba)?;
    Some(Arc::new(RenderImage::new(vec![::image::Frame::new(
        buffer,
    )])))
}

/// An encoded image (PNG, JPEG, ...) decoded for drawing.
pub fn decode(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let rgba = ::image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    raster(w, h, rgba.into_raw())
}

/// A theme icon `name`, a path to an image, or a `file://` URL. `extra_dir` is searched first
/// (a tray item's own icon directory).
pub fn by_name(name: &str, extra_dir: Option<&str>) -> Option<Image> {
    let path = path_for(name, extra_dir)?;
    load(&path)
}

/// Where `by_name` would load from.
pub fn path_for(name: &str, extra_dir: Option<&str>) -> Option<PathBuf> {
    let name = name.strip_prefix("file://").unwrap_or(name);
    if Path::new(name).is_absolute() {
        return Path::new(name).exists().then(|| PathBuf::from(name));
    }
    find(name, extra_dir).or_else(|| find(&format!("{name}-symbolic"), extra_dir))
}

pub fn load(path: &Path) -> Option<Image> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("svg") => Some(Image::Svg(path.to_path_buf())),
        _ => {
            let rgba = ::image::open(path).ok()?.to_rgba8();
            let (w, h) = rgba.dimensions();
            raster(w, h, rgba.into_raw()).map(Image::Raster)
        }
    }
}

fn find(name: &str, extra_dir: Option<&str>) -> Option<PathBuf> {
    if let Some(dir) = extra_dir.filter(|d| !d.is_empty()) {
        for ext in ["svg", "png"] {
            let path = Path::new(dir).join(format!("{name}.{ext}"));
            if path.exists() {
                return Some(path);
            }
        }
    }
    let gtk = freedesktop_icons::default_theme_gtk();
    ["breeze-dark"]
        .into_iter()
        .chain(gtk.as_deref())
        .find_map(|theme| {
            freedesktop_icons::lookup(name)
                .with_size(32)
                .with_theme(theme)
                .with_cache()
                .find()
        })
}

/// `rgba` with its colour pulled `amount` (0–1) of the way to grey.
pub fn desaturate(mut rgba: Vec<u8>, amount: f32) -> Vec<u8> {
    for px in rgba.as_chunks_mut::<4>().0 {
        let [r, g, b] = [px[0], px[1], px[2]].map(f32::from);
        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        for (c, v) in px[..3].iter_mut().zip([r, g, b]) {
            *c = (v + (lum - v) * amount).round() as u8;
        }
    }
    rgba
}
