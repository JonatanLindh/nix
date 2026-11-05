//! Design tokens: the "Glass" look from Quickshell's Theme.qml. Translucent surfaces over the
//! Hyprland-blurred desktop, Geist, near-white highlights.

// The full palette; not every token has a user yet.
#![allow(dead_code)]

use gpui::{FontWeight, Rgba};

const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xff) as f32 / 255.,
        g: ((hex >> 8) & 0xff) as f32 / 255.,
        b: (hex & 0xff) as f32 / 255.,
        a: 1.,
    }
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba {
        r: r as f32 / 255.,
        g: g as f32 / 255.,
        b: b as f32 / 255.,
        a,
    }
}

/// `color` with its alpha replaced.
pub const fn alpha(color: Rgba, a: f32) -> Rgba {
    Rgba { a, ..color }
}

pub const TRANSPARENT: Rgba = rgba(0, 0, 0, 0.);

// Text, from strongest to faintest.
pub const TEXT: Rgba = rgb(0xeef4f3);
pub const TEXT_DIM: Rgba = rgba(235, 243, 242, 0.68);
pub const TEXT_FAINT: Rgba = rgba(235, 243, 242, 0.45);

// Surfaces. The bar is lighter glass than the panels floating over windows.
pub const BAR_BG: Rgba = rgba(8, 18, 22, 0.46);
pub const GLASS: Rgba = rgba(14, 26, 30, 0.62);
pub const GLASS_BORDER: Rgba = rgba(255, 255, 255, 0.14);
// Fills on glass: resting, hover, pressed/open.
pub const FILL: Rgba = rgba(255, 255, 255, 0.08);
pub const FILL_HOVER: Rgba = rgba(255, 255, 255, 0.12);
pub const FILL_ACTIVE: Rgba = rgba(255, 255, 255, 0.18);
pub const DIVIDER: Rgba = rgba(255, 255, 255, 0.08);
pub const BAR_DIVIDER: Rgba = rgba(255, 255, 255, 0.16);
/// Off switches and empty slider tracks.
pub const TRACK: Rgba = rgba(255, 255, 255, 0.14);
/// The session menu's dimmed screen.
pub const BACKDROP: Rgba = rgba(4, 10, 12, 0.42);

/// Solid near-white (slider fills, selected tiles) and the ink drawn on it.
pub const WHITE: Rgba = rgb(0xf4f7f6);
pub const INK: Rgba = rgb(0x0b1d20);
/// "On" highlight. Deliberately the same near-white, not a colour.
pub const ACCENT: Rgba = WHITE;

// Status colours.
pub const WARN: Rgba = rgb(0xff6b6b);
pub const WARN_TEXT: Rgba = rgb(0xff8a8a);
pub const ONLINE: Rgba = rgb(0x3ddc78);
pub const BLUETOOTH: Rgba = rgb(0x5aa9ff);
/// The privacy indicators: camera and microphone in use.
pub const CAMERA: Rgba = rgb(0x3ddc78);
pub const MIC: Rgba = rgb(0xffa94d);
/// Screen sharing, and do-not-disturb.
pub const SCREEN: Rgba = rgb(0xb79cff);

// Type. Regular Geist reads thin on glass, so Medium is the body weight.
pub const FONT: &str = "Geist";
/// For numbers, addresses and key caps.
pub const MONO: &str = "Geist Mono";
pub const WEIGHT: FontWeight = FontWeight::MEDIUM;
pub const SEMIBOLD: FontWeight = FontWeight::SEMIBOLD;
pub const FONT_SIZE: f32 = 13.;
pub const SMALL: f32 = 12.;
pub const TITLE: f32 = 15.;

// Geometry, in logical pixels.
pub const BAR_HEIGHT: f32 = 34.;
pub const BAR_PADDING: f32 = 8.;
pub const PANEL_WIDTH: f32 = 360.;
pub const PANEL_PADDING: f32 = 14.;
/// Space between the bar and a panel hanging under it.
pub const PANEL_GAP: f32 = 8.;
pub const PANEL_RADIUS: f32 = 22.;
pub const BUTTON_RADIUS: f32 = 8.;
pub const GAP: f32 = 10.;
