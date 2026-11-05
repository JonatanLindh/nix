//! Wayland protocols gpui doesn't speak for us, on gpui's own connection.
//!
//! gpui windows hand out their `wl_display` and `wl_surface` through raw-window-handle. This
//! wraps that display in a second wayland-client connection with its own event queue (dispatched
//! on a thread of its own) and uses the surfaces directly. That works for protocols that only
//! need a surface to point at, like the blur region here; anything that changes a surface's
//! role, or must be set before its first commit, stays gpui's business.
//!
//! - **Blur regions**: gpui blurs a whole window or nothing. The notification popups and the
//!   shelf are fixed-size windows with cards in them, so each card gets its own rounded blur
//!   region through `ext-background-effect-v1`.
//! - **Focus grabs**: a popup's own grab needs the press that opened it, which a keybind doesn't
//!   have. Hyprland's `hyprland-focus-grab-v1` keeps input on a set of surfaces without one, and
//!   says when a click elsewhere ends it (HyprlandFocusGrab in Quickshell).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use futures::channel::oneshot;

use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_compositor, wl_region, wl_registry, wl_surface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, delegate_noop};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols_hyprland::focus_grab::v1::client::{
    hyprland_focus_grab_manager_v1::HyprlandFocusGrabManagerV1,
    hyprland_focus_grab_v1::{self, HyprlandFocusGrabV1},
};

struct Wayland {
    conn: Connection,
    qh: QueueHandle<State>,
    compositor: wl_compositor::WlCompositor,
    effects: Option<ExtBackgroundEffectManagerV1>,
    grabs: Option<HyprlandFocusGrabManagerV1>,
    /// Each surface's background-effect object, by the surface's id.
    blurs: Mutex<HashMap<ObjectId, ExtBackgroundEffectSurfaceV1>>,
}

/// Dispatch state for our queue. Events that matter carry their own sender as user data.
struct State;

static WAYLAND: OnceLock<Option<Wayland>> = OnceLock::new();

/// Our side of the connection, set up from the first window that asks.
fn wayland(window: &Window) -> Option<&'static Wayland> {
    WAYLAND
        .get_or_init(|| {
            connect(window)
                .inspect_err(|e| eprintln!("wayland: {e:#}"))
                .ok()
        })
        .as_ref()
}

fn connect(window: &Window) -> anyhow::Result<Wayland> {
    let RawDisplayHandle::Wayland(handle) = HasDisplayHandle::display_handle(window)
        .map_err(|e| anyhow::anyhow!("display handle: {e}"))?
        .as_raw()
    else {
        anyhow::bail!("not on Wayland");
    };
    // SAFETY: the display is gpui's, which lives as long as the app.
    let backend = unsafe { Backend::from_foreign_display(handle.display.as_ptr().cast()) };
    let conn = Connection::from_backend(backend);
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();
    let compositor = globals.bind(&qh, 1..=6, ())?;
    let effects = globals.bind(&qh, 1..=1, ()).ok();
    if effects.is_none() {
        eprintln!("wayland: no ext-background-effect, so no blur behind cards");
    }
    let grabs = globals.bind(&qh, 1..=1, ()).ok();
    if grabs.is_none() {
        eprintln!("wayland: no hyprland-focus-grab, so keybind panels only close when toggled");
    }
    std::thread::Builder::new()
        .name("wayland".into())
        .spawn(move || {
            let mut state = State;
            loop {
                if let Err(e) = queue.blocking_dispatch(&mut state) {
                    eprintln!("wayland: queue stopped: {e}");
                    break;
                }
            }
        })?;
    Ok(Wayland {
        conn,
        qh,
        compositor,
        effects,
        grabs,
        blurs: Mutex::default(),
    })
}

/// The window's `wl_surface`, as an object on our connection.
fn surface(wayland: &Wayland, window: &Window) -> Option<wl_surface::WlSurface> {
    let RawWindowHandle::Wayland(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw()
    else {
        return None;
    };
    // SAFETY: the surface is alive while its window is, and we only use it while handling it.
    let id = unsafe {
        ObjectId::from_ptr(
            wl_surface::WlSurface::interface(),
            handle.surface.as_ptr().cast(),
        )
    }
    .ok()?;
    wl_surface::WlSurface::from_id(&wayland.conn, id).ok()
}

/// Blurs behind each rounded rectangle `(bounds, radius)` of `window`, and nowhere else. Takes
/// effect with the window's next frame.
pub fn set_blur(window: &mut Window, regions: &[(Bounds<Pixels>, f32)]) {
    let Some(wayland) = wayland(window) else {
        return;
    };
    let (Some(effects), Some(surface)) = (&wayland.effects, surface(wayland, window)) else {
        return;
    };
    let mut blurs = wayland.blurs.lock().unwrap();
    let blur = blurs
        .entry(surface.id())
        .or_insert_with(|| effects.get_background_effect(&surface, &wayland.qh, ()));
    let region = wayland.compositor.create_region(&wayland.qh, ());
    for (bounds, radius) in regions {
        for (x, y, w, h) in rounded(*bounds, *radius) {
            region.add(x, y, w, h);
        }
    }
    blur.set_blur_region(Some(&region));
    region.destroy();
    drop(blurs);
    let _ = wayland.conn.flush();
    // The region is double-buffered: it applies when gpui next commits the surface.
    window.refresh();
}

/// Forgets `window`'s blur, before the window closes.
pub fn forget(window: &Window) {
    let Some(wayland) = WAYLAND.get().and_then(Option::as_ref) else {
        return;
    };
    if let Some(surface) = surface(wayland, window)
        && let Some(blur) = wayland.blurs.lock().unwrap().remove(&surface.id())
    {
        blur.destroy();
    }
}

/// Input kept on a set of windows until a click outside them (or another grab) clears it. Add
/// the windows, then `commit` to start it; dropping it ends it.
pub struct FocusGrab {
    grab: HyprlandFocusGrabV1,
}

/// A new, empty focus grab, and what resolves when Hyprland clears it. `None` without the
/// protocol (not Hyprland).
pub fn focus_grab(window: &Window) -> Option<(FocusGrab, oneshot::Receiver<()>)> {
    let wayland = wayland(window)?;
    let (cleared, on_cleared) = oneshot::channel();
    let grab = wayland
        .grabs
        .as_ref()?
        .create_grab(&wayland.qh, Mutex::new(Some(cleared)));
    Some((FocusGrab { grab }, on_cleared))
}

impl FocusGrab {
    /// Lets `window` keep input while the grab holds. Takes effect on `commit`.
    pub fn add(&self, window: &Window) {
        if let Some(wayland) = wayland(window)
            && let Some(surface) = surface(wayland, window)
        {
            self.grab.add_surface(&surface);
        }
    }

    pub fn commit(&self) {
        self.grab.commit();
        if let Some(wayland) = WAYLAND.get().and_then(Option::as_ref) {
            let _ = wayland.conn.flush();
        }
    }
}

impl Drop for FocusGrab {
    fn drop(&mut self) {
        self.grab.destroy();
        if let Some(wayland) = WAYLAND.get().and_then(Option::as_ref) {
            let _ = wayland.conn.flush();
        }
    }
}

/// A rounded rectangle as plain rectangles, which is all a region holds: one per pixel row
/// while the corners curve, then one for the straight middle.
fn rounded(bounds: Bounds<Pixels>, radius: f32) -> Vec<(i32, i32, i32, i32)> {
    let x = f32::from(bounds.origin.x).round() as i32;
    let y = f32::from(bounds.origin.y).round() as i32;
    let width = f32::from(bounds.size.width).round() as i32;
    let height = f32::from(bounds.size.height).round() as i32;
    let r = radius
        .min(width as f32 / 2.)
        .min(height as f32 / 2.)
        .max(0.);
    let rows = r.ceil() as i32;
    let mut out = Vec::with_capacity(2 * rows as usize + 1);
    for row in 0..rows {
        // How far in the curve is at this row's middle.
        let dy = r - (row as f32 + 0.5);
        let inset = (r - (r * r - dy * dy).max(0.).sqrt()).round() as i32;
        let w = width - 2 * inset;
        out.push((x + inset, y + row, w, 1));
        out.push((x + inset, y + height - 1 - row, w, 1));
    }
    out.push((x, y + rows, width, height - 2 * rows));
    out
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        _: ext_background_effect_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HyprlandFocusGrabV1, Mutex<Option<oneshot::Sender<()>>>> for State {
    fn event(
        _: &mut Self,
        _: &HyprlandFocusGrabV1,
        event: hyprland_focus_grab_v1::Event,
        cleared: &Mutex<Option<oneshot::Sender<()>>>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let hyprland_focus_grab_v1::Event::Cleared = event
            && let Some(cleared) = cleared.lock().unwrap().take()
        {
            let _ = cleared.send(());
        }
    }
}

delegate_noop!(State: ignore HyprlandFocusGrabManagerV1);
delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_region::WlRegion);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);
