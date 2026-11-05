//! The capture overlay's arithmetic (ScreenshotOverlay.qml): the selection and its drags, the
//! window under the pointer, the dimmed frame around the target, and the names and labels the
//! captures get. Kept free of gpui so it can be tested.

use chrono::{DateTime, Local};

/// A rectangle in logical pixels: a monitor's own (local) or the global layout's.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Whether the point is inside, the right and bottom edges excluded.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    /// The same rectangle moved by `(dx, dy)`.
    pub fn offset(&self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

/// What a press on the overlay does to the selection as the pointer moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drag {
    /// Draws a new selection from the press.
    New,
    /// Moves the selection, keeping its size.
    Move,
    /// Moves one corner; the opposite one stays.
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// How near a corner (in either direction) a press grabs it.
const HANDLE_REACH: f32 = 12.;
/// A selection smaller than this either way is a stray click, not a selection.
const MIN_SELECTION: f32 = 4.;

/// What a press at `(px, py)` grabs of `sel`: a corner, the inside, or nothing (`None`, so a new
/// selection starts).
pub fn handle_at(sel: Option<Rect>, px: f32, py: f32) -> Option<Drag> {
    let s = sel?;
    let near =
        |ax: f32, ay: f32| (px - ax).abs() <= HANDLE_REACH && (py - ay).abs() <= HANDLE_REACH;
    if near(s.x, s.y) {
        Some(Drag::TopLeft)
    } else if near(s.x + s.w, s.y) {
        Some(Drag::TopRight)
    } else if near(s.x, s.y + s.h) {
        Some(Drag::BottomLeft)
    } else if near(s.x + s.w, s.y + s.h) {
        Some(Drag::BottomRight)
    } else if px > s.x && px < s.x + s.w && py > s.y && py < s.y + s.h {
        Some(Drag::Move)
    } else {
        None
    }
}

/// The selection while dragging: `start` is where the press was, `from` the selection then (a
/// zero-size one at the press for [`Drag::New`]), `(mx, my)` the pointer now, and `(width,
/// height)` the monitor, which the selection stays inside.
pub fn dragged(
    drag: Drag,
    start: (f32, f32),
    from: Rect,
    (mx, my): (f32, f32),
    (width, height): (f32, f32),
) -> Rect {
    let (dx, dy) = (mx - start.0, my - start.1);
    let clamp_x = |v: f32| v.clamp(0., width);
    let clamp_y = |v: f32| v.clamp(0., height);
    if drag == Drag::Move {
        return Rect::new(
            (from.x + dx).clamp(0., (width - from.w).max(0.)),
            (from.y + dy).clamp(0., (height - from.h).max(0.)),
            from.w,
            from.h,
        );
    }
    let (mut x1, mut y1, mut x2, mut y2) = (from.x, from.y, from.x + from.w, from.y + from.h);
    match drag {
        Drag::New => {
            x2 = clamp_x(mx);
            y2 = clamp_y(my);
        }
        _ => {
            if matches!(drag, Drag::TopLeft | Drag::BottomLeft) {
                x1 = clamp_x(from.x + dx);
            }
            if matches!(drag, Drag::TopRight | Drag::BottomRight) {
                x2 = clamp_x(from.x + from.w + dx);
            }
            if matches!(drag, Drag::TopLeft | Drag::TopRight) {
                y1 = clamp_y(from.y + dy);
            }
            if matches!(drag, Drag::BottomLeft | Drag::BottomRight) {
                y2 = clamp_y(from.y + from.h + dy);
            }
        }
    }
    Rect::new(x1.min(x2), y1.min(y2), (x2 - x1).abs(), (y2 - y1).abs())
}

/// The selection once the press is let go: a click without dragging leaves none behind.
pub fn settled(sel: Option<Rect>) -> Option<Rect> {
    sel.filter(|s| s.w >= MIN_SELECTION && s.h >= MIN_SELECTION)
}

/// The topmost of `windows` (topmost first) under `(px, py)`.
pub fn window_at(windows: &[Rect], px: f32, py: f32) -> Option<Rect> {
    windows.iter().copied().find(|w| w.contains(px, py))
}

/// The four dimmed bands around `cut` on a `width` × `height` monitor: above, below, left and
/// right of it. With nothing cut out (a zero rectangle) the band above is empty and the one below
/// covers everything.
pub fn dim_bands(cut: Rect, width: f32, height: f32) -> [Rect; 4] {
    [
        Rect::new(0., 0., width, cut.y),
        Rect::new(0., cut.y + cut.h, width, height - cut.y - cut.h),
        Rect::new(0., cut.y, cut.x, cut.h),
        Rect::new(cut.x + cut.w, cut.y, width - cut.x - cut.w, cut.h),
    ]
}

/// Where the size label goes: just under the selection, or above it when there's no room below.
pub fn label_top(cut: Rect, label_height: f32, height: f32) -> f32 {
    let below = cut.y + cut.h + 10.;
    if below + label_height < height {
        below
    } else {
        cut.y - label_height - 10.
    }
}

/// "1280 × 720", the selection's size in logical pixels.
pub fn size_label(cut: Rect) -> String {
    format!("{} × {}", cut.w.round(), cut.h.round())
}

/// `target` (local logical pixels) in the pixels of a still `scale` times the monitor's logical
/// size, kept inside the still's `(width, height)`: `(x, y, w, h)`.
pub fn crop(target: Rect, scale: f32, (width, height): (u32, u32)) -> (u32, u32, u32, u32) {
    let r = |v: f32| (v * scale).round().max(0.) as u32;
    let (x, y) = (r(target.x).min(width), r(target.y).min(height));
    let w = r(target.w).min(width - x);
    let h = r(target.h).min(height - y);
    (x, y, w, h)
}

/// A global rectangle as wf-recorder's `-g`: "x,y wxh".
pub fn geometry(rect: Rect) -> String {
    format!(
        "{},{} {}x{}",
        rect.x.round(),
        rect.y.round(),
        rect.w.round(),
        rect.h.round()
    )
}

/// "2026-10-19 20-31-07", for "Screenshot from …" like GNOME.
pub fn stamp(now: DateTime<Local>) -> String {
    now.format("%Y-%m-%d %H-%M-%S").to_string()
}

/// "4:07": a recording's minutes and seconds so far.
pub fn elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const SEL: Rect = Rect::new(100., 100., 200., 100.);
    const MONITOR: (f32, f32) = (1000., 800.);

    #[test]
    fn grabs_corners_inside_or_nothing() {
        assert_eq!(handle_at(None, 5., 5.), None);
        assert_eq!(handle_at(Some(SEL), 95., 108.), Some(Drag::TopLeft));
        assert_eq!(handle_at(Some(SEL), 305., 95.), Some(Drag::TopRight));
        assert_eq!(handle_at(Some(SEL), 100., 200.), Some(Drag::BottomLeft));
        assert_eq!(handle_at(Some(SEL), 311., 211.), Some(Drag::BottomRight));
        assert_eq!(handle_at(Some(SEL), 200., 150.), Some(Drag::Move));
        assert_eq!(handle_at(Some(SEL), 50., 50.), None);
    }

    #[test]
    fn new_selection_follows_the_pointer_either_way() {
        let from = Rect::new(500., 400., 0., 0.);
        let r = dragged(Drag::New, (500., 400.), from, (300., 600.), MONITOR);
        assert_eq!(r, Rect::new(300., 400., 200., 200.));
        // Past the monitor's edge, it stops there.
        let r = dragged(Drag::New, (500., 400.), from, (1200., -50.), MONITOR);
        assert_eq!(r, Rect::new(500., 0., 500., 400.));
    }

    #[test]
    fn move_keeps_size_and_stays_on_the_monitor() {
        let r = dragged(Drag::Move, (150., 150.), SEL, (170., 140.), MONITOR);
        assert_eq!(r, Rect::new(120., 90., 200., 100.));
        let r = dragged(Drag::Move, (150., 150.), SEL, (2000., 2000.), MONITOR);
        assert_eq!(r, Rect::new(800., 700., 200., 100.));
    }

    #[test]
    fn corner_drag_moves_one_corner_and_can_flip() {
        let r = dragged(Drag::BottomRight, (300., 200.), SEL, (350., 260.), MONITOR);
        assert_eq!(r, Rect::new(100., 100., 250., 160.));
        // Dragging the top-left corner past the bottom-right one flips the selection.
        let r = dragged(Drag::TopLeft, (100., 100.), SEL, (400., 250.), MONITOR);
        assert_eq!(r, Rect::new(300., 200., 100., 50.));
    }

    #[test]
    fn a_click_leaves_no_selection() {
        assert_eq!(settled(Some(Rect::new(1., 1., 3., 50.))), None);
        assert_eq!(settled(Some(SEL)), Some(SEL));
        assert_eq!(settled(None), None);
    }

    #[test]
    fn topmost_window_wins() {
        let windows = [
            Rect::new(0., 0., 100., 100.),
            Rect::new(50., 50., 200., 200.),
        ];
        assert_eq!(window_at(&windows, 60., 60.), Some(windows[0]));
        assert_eq!(window_at(&windows, 150., 150.), Some(windows[1]));
        assert_eq!(window_at(&windows, 300., 10.), None);
    }

    #[test]
    fn bands_cover_everything_but_the_cut() {
        let bands = dim_bands(SEL, 1000., 800.);
        let area: f32 = bands.iter().map(|b| b.w * b.h).sum();
        assert_eq!(area, 1000. * 800. - SEL.w * SEL.h);
        let none = dim_bands(Rect::default(), 1000., 800.);
        assert_eq!(none[1], Rect::new(0., 0., 1000., 800.));
    }

    #[test]
    fn label_goes_above_when_no_room_below() {
        assert_eq!(label_top(SEL, 24., 800.), 210.);
        assert_eq!(label_top(Rect::new(0., 700., 10., 80.), 24., 800.), 666.);
    }

    #[test]
    fn crop_scales_and_stays_inside() {
        let still = (3840, 2160);
        assert_eq!(crop(SEL, 1.5, still), (150, 150, 300, 150));
        let edge = Rect::new(2500., 1400., 100., 100.);
        assert_eq!(crop(edge, 1.5, still), (3750, 2100, 90, 60));
    }

    #[test]
    fn names_and_labels() {
        assert_eq!(
            geometry(Rect::new(2560.4, 0., 640.6, 480.)),
            "2560,0 641x480"
        );
        assert_eq!(size_label(Rect::new(0., 0., 1279.6, 720.2)), "1280 × 720");
        assert_eq!(elapsed(247), "4:07");
        assert_eq!(elapsed(5), "0:05");
        let t = Local.with_ymd_and_hms(2026, 10, 19, 20, 31, 7).unwrap();
        assert_eq!(stamp(t), "2026-10-19 20-31-07");
    }
}
