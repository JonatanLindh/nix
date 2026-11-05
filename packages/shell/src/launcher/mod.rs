//! The launcher (Launcher.qml, with Clip.qml and Passwords.qml): apps, and through a prefix the
//! other modes, shown as a chip in front of the search field (backspace on an empty field drops
//! it again):
//!
//! - `>cmd` runs a shell command, `=1+2` calculates with qalc (enter copies, shift+enter opens
//!   Qalculate)
//! - `~` files in home, `d~` files in ~/Downloads (newest first), `/` files anywhere
//! - `"` clipboard history (also `shell msg clip`), `*` 1Password (also `shell msg op`)
//!
//! A layer window of its own that takes the keyboard while open and closes on a click
//! elsewhere (a Hyprland focus grab). Apps start through uwsm, each in its own scope. Files
//! open, show in Files, copy their path or go to the shelf (the same modifiers on enter and
//! click), and can be dragged out into other apps.
//!
//! The searching is in `services::launcher`: `find` ranks apps and 1Password items, searches
//! files and runs the calculator; `clip` reads clipboard history; `passwords` reads 1Password.
//! This entity keeps the state (it outlives the window, so the lists are there at once on the
//! next open); `view` draws it.

mod apps;
mod formula;
mod view;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures::StreamExt;
use gpui::component::Root;
use gpui::{
    App, AppContext, Context, Entity, Global, Modifiers, Size, Subscription, Task,
    WindowBackgroundAppearance, WindowHandle,
    layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions},
    px,
};

use crate::runtime;
use crate::services::icons::{self, Image};
use crate::services::launcher::clip::{self, Entry, Listed};
use crate::services::launcher::find::calc::{self, Answer as Calc};
use crate::services::launcher::find::files::{self, File};
use crate::services::launcher::find::rank::{self, List};
use crate::services::launcher::passwords::{self, Field, Item};
use crate::services::spawn;
use crate::shelf::Shelf;
use crate::shell::{close_window, display_named, open_layer};
use crate::wayland::{self, FocusGrab};
use apps::App as DesktopApp;

/// The window: fixed, since resizing a layer surface on every keystroke makes Hyprland animate
/// it. The card sizes itself inside, and only the card takes input.
const WIDTH: f32 = 720.;
const HEIGHT: f32 = 620.;
const TOP: f32 = 110.;
/// File searches wait for a pause in typing.
const DEBOUNCE: Duration = Duration::from_millis(140);
/// A grab taken in the same moment the window opens gets cleared straight away.
const GRAB_DELAY: Duration = Duration::from_millis(50);
/// Loading for this long means 1Password is showing its authorization dialog.
const SLOW_LOAD: Duration = Duration::from_millis(1500);
/// The selected item's fields are fetched once the selection settles.
const DETAIL_DELAY: Duration = Duration::from_millis(250);
/// 1Password items ranked at most.
const PASS_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Apps,
    Run,
    Calc,
    Files,
    Clip,
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Home,
    Downloads,
    Root,
}

impl Scope {
    fn name(self) -> &'static str {
        match self {
            Scope::Home => "home",
            Scope::Downloads => "downloads",
            Scope::Root => "root",
        }
    }
}

/// The mode a prefix typed at the start of the apps search switches to.
fn prefix(c: char) -> Option<(Mode, Scope)> {
    Some(match c {
        '>' => (Mode::Run, Scope::Home),
        '=' => (Mode::Calc, Scope::Home),
        '"' => (Mode::Clip, Scope::Home),
        '*' => (Mode::Pass, Scope::Home),
        '~' => (Mode::Files, Scope::Home),
        '/' => (Mode::Files, Scope::Root),
        _ => return None,
    })
}

/// The mode and the rest of the text, when `text` starts with a prefix. Downloads is "d~";
/// after the prefix everything typed is search text.
fn split_prefix(text: &str) -> Option<(Mode, Scope, &str)> {
    if let Some(rest) = text.strip_prefix("d~") {
        return Some((Mode::Files, Scope::Downloads, rest));
    }
    let c = text.chars().next()?;
    prefix(c).map(|(mode, scope)| (mode, scope, &text[c.len_utf8()..]))
}

/// Enter's or a click's modifiers, as the actions read them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl From<Modifiers> for Mods {
    fn from(m: Modifiers) -> Self {
        Self {
            shift: m.shift,
            ctrl: m.control,
            alt: m.alt,
        }
    }
}

/// Clipboard history (Clip.qml).
#[derive(Default)]
struct ClipState {
    entries: Vec<Entry>,
    /// Indices of the entries matching the search.
    shown: Vec<usize>,
    /// From the full entry, decoded on demand for the preview pane: the text (at most 8000
    /// bytes; "" for images and failures) and the meta.
    texts: HashMap<String, String>,
    metas: HashMap<String, String>,
    /// Images whose thumbnail is written.
    thumbs: HashSet<String>,
    wanted: Option<String>,
    previewing: bool,
    _listing: Option<Task<()>>,
}

/// 1Password (Passwords.qml). Item metadata stays in memory between opens, so the list shows at
/// once, and is refreshed in the background on every open.
#[derive(Default)]
struct PassState {
    items: Vec<Item>,
    /// The items in the search's order, as indices.
    ranked: Vec<usize>,
    /// Waiting for the first list (nothing to show yet).
    loading: bool,
    listing: bool,
    /// Loading for a while means 1Password shows its authorization dialog: the launcher lets
    /// go of the keyboard so it can be typed into.
    authorizing: bool,
    error: Option<String>,
    /// The item whose fields are shown (the field view).
    item: Option<Item>,
    /// Indices of its fields matching the search.
    shown_fields: Vec<usize>,
    /// Field metadata per item id, for the detail pane and the field view; cleared on every
    /// open, like the list.
    fields: HashMap<String, Vec<Field>>,
    want: Option<Item>,
    fetching: bool,
    /// The item list's search and selection, put back when leaving an item's fields.
    list_query: String,
    list_sel: usize,
    _slow_load: Option<Task<()>>,
    _detail_delay: Option<Task<()>>,
}

impl PassState {
    fn fields_of(&self, item: &Item) -> Option<&Vec<Field>> {
        self.fields.get(&item.id)
    }

    /// Whether it has a one-time code; `None` while its fields aren't known.
    fn has_otp(&self, item: &Item) -> Option<bool> {
        self.fields_of(item).map(|f| f.iter().any(Field::is_otp))
    }
}

/// A row of the current mode's list.
pub enum Row<'a> {
    App(&'a DesktopApp),
    File(&'a File),
    Clip(&'a Entry),
    Pass(&'a Item),
    Field(&'a Field),
}

pub struct Launcher {
    /// Logically open (the window may still be opening).
    open: bool,
    window: Option<WindowHandle<Root>>,
    /// What the window was opened as: the launcher, `clip` or `op`.
    opened_as: Mode,
    mode: Mode,
    scope: Scope,
    query: String,
    /// Text for the search field to take: cleared, or a prefix taken off.
    pending_text: Option<String>,
    /// Selected row of the current mode's list.
    sel: usize,
    /// A file is being dragged out: the card fades and lets the pointer through.
    dragging: bool,
    grab: Option<FocusGrab>,
    grab_ready: bool,
    /// Whether the window has the keyboard, as last set.
    keyboard: bool,
    shelf: Entity<Shelf>,

    apps: Vec<DesktopApp>,
    app_index: HashMap<String, usize>,
    /// App icons by icon name; `None` for those the theme hasn't got (a letter tile instead).
    icons: HashMap<String, Option<Image>>,
    shown_apps: Vec<usize>,
    /// The most used apps, shown above the list while the search is empty.
    recent: Vec<usize>,

    /// The apps and 1Password items to rank, with their launch counts.
    apps_list: List,
    pass_list: List,
    /// Calculations and file searches carry a sequence number and only the answer to the latest
    /// of each kind is used.
    seq: u64,
    calc_seq: u64,
    files_seq: u64,
    files_done: u64,
    /// The running calculation, stopped when a newer one starts.
    calc_task: Option<tokio::task::AbortHandle>,
    /// The latest file search's number, which older ones watch to stop.
    files_latest: Arc<AtomicU64>,
    calc: Option<Calc>,
    math: Option<formula::Node>,
    files: Vec<File>,
    files_total: usize,
    files_capped: bool,
    debounce: Option<Task<()>>,

    clip: ClipState,
    pw: PassState,
    /// What the selection was on, to notice it moving.
    last_current: Option<String>,
    /// Watches the open window's view going away.
    _released: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

struct GlobalLauncher(Entity<Launcher>);

impl Global for GlobalLauncher {}

/// Starts the launcher's search in the background; the window opens with `toggle`.
pub fn init(cx: &mut App) {
    let shelf = crate::shell::Shell::global(cx).read(cx).shelf.clone();
    let launcher = cx.new(|cx| Launcher::new(shelf, cx));
    cx.set_global(GlobalLauncher(launcher));
}

/// `shell msg launcher | clip | op`: opens on `monitor` in that mode, closes if it's open in
/// it, or switches to it.
pub fn toggle(command: &str, monitor: Option<String>, cx: &mut App) {
    let mode = match command {
        "clip" => Mode::Clip,
        "op" => Mode::Pass,
        _ => Mode::Apps,
    };
    if let Some(launcher) = cx.try_global::<GlobalLauncher>().map(|g| g.0.clone()) {
        launcher.update(cx, |l, cx| l.toggle(mode, monitor, cx));
    }
}

/// `$XDG_STATE_HOME/shell/<name>`, started from Quickshell's copy if there's one, so launch
/// counts carry over.
fn state_file(name: &str) -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let path = state.join("shell").join(name);
    if !path.exists() {
        let old = std::fs::read_dir(state.join("quickshell/by-shell"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|d| d.path().join(name))
            .find(|p| p.exists());
        if let Some(old) = old {
            let _ = std::fs::create_dir_all(state.join("shell"));
            let _ = std::fs::copy(old, &path);
        }
    }
    path
}

/// An app icon from the theme, small enough to keep many of in memory.
fn load_icon(name: &str) -> Option<Image> {
    let path = icons::path_for(name, None)?;
    if path.extension().is_some_and(|e| e == "svg") {
        return Some(Image::Svg(path));
    }
    let mut image = ::image::open(&path).ok()?;
    if image.width() > 96 || image.height() > 96 {
        image = image.thumbnail(96, 96);
    }
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    icons::raster(w, h, rgba.into_raw()).map(Image::Raster)
}

impl Launcher {
    fn new(shelf: Entity<Shelf>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&shelf, |_, _, cx| cx.notify())];
        let mut launcher = Self {
            open: false,
            window: None,
            opened_as: Mode::Apps,
            mode: Mode::Apps,
            scope: Scope::Home,
            query: String::new(),
            pending_text: None,
            sel: 0,
            dragging: false,
            grab: None,
            grab_ready: false,
            keyboard: true,
            shelf,
            apps: Vec::new(),
            app_index: HashMap::new(),
            icons: HashMap::new(),
            shown_apps: Vec::new(),
            recent: Vec::new(),
            apps_list: List::new(Some(state_file("launcher.json"))),
            pass_list: List::new(Some(state_file("passwords.json"))),
            seq: 0,
            calc_seq: 0,
            files_seq: 0,
            files_done: 0,
            calc_task: None,
            files_latest: Arc::default(),
            calc: None,
            math: None,
            files: Vec::new(),
            files_total: 0,
            files_capped: false,
            debounce: None,
            clip: ClipState::default(),
            pw: PassState::default(),
            last_current: None,
            _released: None,
            _subscriptions: subscriptions,
        };
        launcher.scan_apps(cx);
        launcher
    }

    // ---- Opening and closing

    fn toggle(&mut self, mode: Mode, monitor: Option<String>, cx: &mut Context<Self>) {
        if self.open {
            if self.opened_as == mode {
                self.close(cx);
                return;
            }
            // Switching between launcher, clip and op while open starts fresh.
            self.opened_as = mode;
            self.start(cx);
            return;
        }
        self.open = true;
        self.opened_as = mode;
        self.start(cx);
        self.scan_apps(cx);
        self.open_window(monitor, cx);
    }

    /// Fresh state for `opened_as`.
    fn start(&mut self, cx: &mut Context<Self>) {
        self.dragging = false;
        self.set_text(String::new());
        self.pw.item = None;
        self.set_mode(self.opened_as, Scope::Home, cx);
        self.changed(cx);
    }

    fn open_window(&mut self, monitor: Option<String>, cx: &mut Context<Self>) {
        let model = cx.entity();
        // After this update: the view reads the launcher as it opens.
        cx.defer(move |cx| {
            let display = monitor.as_deref().and_then(|m| display_named(m, cx));
            let mut view = None;
            let opened = open_layer(
                cx,
                display,
                layer(),
                Size::new(px(WIDTH), px(HEIGHT)),
                WindowBackgroundAppearance::Transparent,
                |window, cx| {
                    let v = cx.new(|cx| view::View::new(model.clone(), window, cx));
                    view = Some(v.clone());
                    v.into()
                },
            );
            model.update(cx, |l, cx| match (opened, view) {
                (Ok(handle), Some(view)) => {
                    if !l.open || l.window.is_some() {
                        // Closed again while it opened.
                        close_window(handle, cx);
                        return;
                    }
                    l.window = Some(handle);
                    l.keyboard = true;
                    l.grab_ready = false;
                    let released = cx.observe_release(&view, move |l, _, cx| {
                        if l.window == Some(handle) {
                            l.window = None;
                            l.grab = None;
                            l.open = false;
                            cx.notify();
                        }
                    });
                    l._released = Some(released);
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(GRAB_DELAY).await;
                        let _ = this.update(cx, |l, cx| {
                            if l.window == Some(handle) {
                                l.grab_ready = true;
                                l.sync_window(cx);
                            }
                        });
                    })
                    .detach();
                }
                (Err(e), _) => {
                    eprintln!("launcher: opening the window: {e:#}");
                    l.open = false;
                }
                (Ok(handle), None) => close_window(handle, cx),
            });
        });
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.dragging = false;
        self.grab = None;
        self.debounce = None;
        if let Some(window) = self.window.take() {
            close_window(window, cx);
        }
        cx.notify();
    }

    /// Hands the keyboard over while 1Password authorizes and back after; keeps the focus grab
    /// on except while 1Password loads (its dialog may need the click). After the current
    /// update: the window can't be changed from inside its own handlers.
    fn sync_window(&mut self, cx: &mut Context<Self>) {
        let this = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = this.update(cx, |l, cx| l.apply_window(cx));
        });
    }

    fn apply_window(&mut self, cx: &mut Context<Self>) {
        let Some(handle) = self.window else {
            return;
        };
        let pass = self.mode == Mode::Pass;
        let keyboard = !(pass && self.pw.authorizing);
        if keyboard != self.keyboard {
            self.keyboard = keyboard;
            let mode = if keyboard {
                KeyboardInteractivity::Exclusive
            } else {
                KeyboardInteractivity::None
            };
            let _ = handle.update(cx, |_, window, _| window.set_keyboard_interactivity(mode));
        }
        let grab = self.grab_ready && !(pass && self.pw.loading);
        if !grab {
            self.grab = None;
        } else if self.grab.is_none() {
            self.grab = self.take_grab(handle, cx);
        }
    }

    /// Closes the launcher when a click outside clears the grab.
    fn take_grab(&self, handle: WindowHandle<Root>, cx: &mut Context<Self>) -> Option<FocusGrab> {
        let (grab, cleared) = handle
            .update(cx, |_, window, _| wayland::focus_grab(window))
            .ok()
            .flatten()?;
        let _ = handle.update(cx, |_, window, _| grab.add(window));
        grab.commit();
        cx.spawn(async move |this, cx| {
            // Cancelled instead when the grab is dropped first.
            if cleared.await.is_ok() {
                let _ = this.update(cx, |l, cx| {
                    if l.window == Some(handle) {
                        l.close(cx);
                    }
                });
            }
        })
        .detach();
        Some(grab)
    }

    // ---- Changes

    /// After every change: the filtered lists, what the selection is on, the window.
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.refilter();
        let current = self.current_key();
        if current != self.last_current {
            self.last_current = current;
            self.current_changed(cx);
        }
        self.sync_window(cx);
        cx.notify();
    }

    fn refilter(&mut self) {
        let q = self.query.trim().to_lowercase();
        self.clip.shown = (self.clip.entries.iter().enumerate())
            .filter(|(_, e)| q.is_empty() || e.text.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        self.pw.shown_fields = match self.pw.item.as_ref().and_then(|i| self.pw.fields_of(i)) {
            Some(fields) => (fields.iter().enumerate())
                .filter(|(_, f)| q.is_empty() || f.label.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect(),
            None => Vec::new(),
        };
    }

    fn current_key(&self) -> Option<String> {
        let id = match self.current()? {
            Row::App(a) => a.id.clone(),
            Row::File(f) => f.path.clone(),
            Row::Clip(e) => e.id.clone(),
            Row::Pass(i) => i.id.clone(),
            Row::Field(f) => f.id.clone(),
        };
        Some(format!("{:?}:{id}", self.mode))
    }

    fn current_changed(&mut self, cx: &mut Context<Self>) {
        match (self.mode, self.current()) {
            (Mode::Clip, Some(Row::Clip(entry))) => {
                let id = entry.id.clone();
                self.preview(id, cx);
            }
            (Mode::Pass, Some(Row::Pass(item))) => {
                let item = item.clone();
                self.pw._detail_delay = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(DETAIL_DELAY).await;
                    let _ = this.update(cx, |l, cx| {
                        if l.mode == Mode::Pass && l.pw.item.is_none() {
                            l.prefetch(item, cx);
                        }
                    });
                }));
            }
            _ => {}
        }
    }

    /// Sets the search text from here (the view puts it in the field).
    fn set_text(&mut self, text: String) {
        self.query = text.clone();
        self.pending_text = Some(text);
    }

    /// The text the search field should take, once.
    pub fn take_text(&mut self) -> Option<String> {
        self.pending_text.take()
    }

    /// The search text was edited: a prefix at the start of the apps search becomes the mode.
    pub fn text_edited(&mut self, text: String, cx: &mut Context<Self>) {
        let mut text = text;
        if self.mode == Mode::Apps
            && let Some((mode, scope, rest)) = split_prefix(&text)
        {
            let rest = rest.to_string();
            self.set_text(rest.clone());
            text = rest;
            self.set_mode(mode, scope, cx);
        }
        self.query = text;
        self.sel = 0;
        if self.mode == Mode::Files {
            self.search_soon(cx);
        }
        self.ask(cx);
        self.changed(cx);
    }

    fn set_mode(&mut self, mode: Mode, scope: Scope, cx: &mut Context<Self>) {
        self.mode = mode;
        self.scope = scope;
        self.sel = 0;
        match mode {
            Mode::Clip => self.clip_load(cx),
            Mode::Pass => self.pw_refresh(cx),
            Mode::Files => self.search_soon(cx),
            _ => {}
        }
        self.ask(cx);
    }

    // ---- Search

    fn set_lists(&mut self) {
        let apps = self
            .apps
            .iter()
            .map(|a| rank::Item {
                id: a.id.clone(),
                name: a.name.clone(),
                hay: a.hay.clone(),
            })
            .collect();
        self.apps_list.set(apps);
        let pass = self
            .pw
            .items
            .iter()
            .map(|i| rank::Item {
                id: i.id.clone(),
                name: i.title.clone(),
                hay: format!("{} {}", i.user, i.urls),
            })
            .collect();
        self.pass_list.set(pass);
    }

    /// Finds what the current mode and query show: ranking at once, a calculation in the
    /// background (files go through the debounce instead).
    fn ask(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        match self.mode {
            Mode::Apps => {
                let (ids, recent) = self.apps_list.rank(&self.query, 0);
                let index = |ids: Vec<String>| -> Vec<usize> {
                    ids.iter()
                        .filter_map(|id| self.app_index.get(id).copied())
                        .collect()
                };
                self.shown_apps = index(ids);
                self.recent = index(recent);
            }
            Mode::Pass if self.pw.item.is_none() => {
                let (ids, _) = self.pass_list.rank(&self.query, PASS_LIMIT);
                let by_id: HashMap<&str, usize> = (self.pw.items.iter().enumerate())
                    .map(|(i, item)| (item.id.as_str(), i))
                    .collect();
                self.pw.ranked = ids
                    .iter()
                    .filter_map(|id| by_id.get(id.as_str()).copied())
                    .collect();
            }
            Mode::Calc => self.calculate(cx),
            _ => {}
        }
    }

    /// qalc on the services runtime; a newer expression stops the one still running.
    fn calculate(&mut self, cx: &mut Context<Self>) {
        self.seq += 1;
        self.calc_seq = self.seq;
        let seq = self.seq;
        if let Some(running) = self.calc_task.take() {
            running.abort();
        }
        let q = self.query.clone();
        let task = runtime::spawn(async move { calc::eval(&q).await });
        self.calc_task = Some(task.abort_handle());
        cx.spawn(async move |this, cx| {
            // An aborted one has no answer; one that finished just before still checks `seq`.
            let Ok(calc) = task.await else { return };
            let _ = this.update(cx, |l, cx| {
                if seq != l.calc_seq {
                    return;
                }
                l.calc_task = None;
                l.math = calc.math.as_ref().map(formula::Node::parse);
                l.calc = Some(calc);
                l.changed(cx);
            });
        })
        .detach();
    }

    /// Reads the desktop entries again (they change as packages do), and their icons.
    fn scan_apps(&mut self, cx: &mut Context<Self>) {
        runtime::then(
            cx,
            async {
                tokio::task::spawn_blocking(apps::scan)
                    .await
                    .unwrap_or_default()
            },
            |l, apps, cx| {
                if apps == l.apps {
                    return;
                }
                l.app_index = (apps.iter().enumerate())
                    .map(|(i, a)| (a.id.clone(), i))
                    .collect();
                l.apps = apps;
                l.shown_apps.clear();
                l.recent.clear();
                l.set_lists();
                l.ask(cx);
                l.load_icons(cx);
                l.changed(cx);
            },
        );
    }

    fn load_icons(&mut self, cx: &mut Context<Self>) {
        let names: Vec<String> = (self.apps.iter())
            .filter_map(|a| a.icon.clone())
            .filter(|n| !self.icons.contains_key(n))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if names.is_empty() {
            return;
        }
        runtime::then(
            cx,
            async move {
                tokio::task::spawn_blocking(move || {
                    names
                        .into_iter()
                        .map(|n| {
                            let icon = load_icon(&n);
                            (n, icon)
                        })
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap_or_default()
            },
            |l, loaded, cx| {
                l.icons.extend(loaded);
                cx.notify();
            },
        );
    }

    // ---- Files

    fn search_soon(&mut self, cx: &mut Context<Self>) {
        self.debounce = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let _ = this.update(cx, |l, cx| {
                l.debounce = None;
                l.run_search(cx);
                l.changed(cx);
            });
        }));
    }

    /// Searches in the background; a newer search stops this one. 50 rows at most; the total
    /// counts up to 400 matches.
    fn run_search(&mut self, cx: &mut Context<Self>) {
        if !self.open || self.mode != Mode::Files {
            return;
        }
        self.seq += 1;
        self.files_seq = self.seq;
        let seq = self.seq;
        let latest = self.files_latest.clone();
        latest.store(seq, Ordering::SeqCst);
        let scope = self.scope.name();
        let q = self.query.trim().to_string();
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/".into()));
        runtime::then(
            cx,
            async move {
                tokio::task::spawn_blocking(move || {
                    let stale = || latest.load(Ordering::SeqCst) != seq;
                    let found = files::search(scope, &q, &home, &stale);
                    (!stale()).then_some(found)
                })
                .await
                .ok()
                .flatten()
            },
            move |l, found, cx| {
                let Some(found) = found.filter(|_| seq == l.files_seq) else {
                    return;
                };
                l.files = found.files;
                l.files_total = found.total;
                l.files_capped = found.capped;
                l.files_done = seq;
                l.sel = 0;
                l.changed(cx);
            },
        );
    }

    pub fn files_busy(&self) -> bool {
        self.mode == Mode::Files && (self.files_seq != self.files_done || self.debounce.is_some())
    }

    pub fn on_shelf(&self, path: &str, cx: &App) -> bool {
        self.shelf.read(cx).items.iter().any(|i| i.path == path)
    }

    /// No modifier opens, shift shows it in Files, ctrl copies the path, alt adds it to the
    /// shelf. The same with enter and with a click.
    fn act_file(&mut self, file: File, mods: Mods, cx: &mut Context<Self>) {
        self.close(cx);
        if mods.alt {
            runtime::detach(crate::services::shelf::add(vec![file.path]));
        } else if mods.ctrl {
            copy(file.path);
        } else if mods.shift {
            // The file manager selects it in its folder; just the folder if none answers.
            let dir = match file.path.rfind('/') {
                Some(0) | None => "/".to_string(),
                Some(i) => file.path[..i].to_string(),
            };
            let script = r#"dbus-send --session --print-reply --dest=org.freedesktop.FileManager1 --type=method_call /org/freedesktop/FileManager1 org.freedesktop.FileManager1.ShowItems "array:string:$1" string: >/dev/null 2>&1 || exec uwsm-app -- xdg-open "$2""#;
            let url = file_url(&file.path);
            runtime::detach(async move {
                spawn("sh", &["-c", script, "sh", &url, &dir]).await;
            });
        } else {
            runtime::detach(async move {
                spawn("uwsm-app", &["--", "xdg-open", &file.path]).await;
            });
        }
    }

    /// A file is being dragged out of the window: hide, so the drop targets underneath show.
    pub fn drag_out(&mut self, cx: &mut Context<Self>) {
        if !self.dragging {
            self.dragging = true;
            cx.notify();
        }
    }

    /// A drag started on a file row; the launcher is done once it ends, wherever it ended.
    pub fn drag_started(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if !cx.update(|cx| cx.has_active_drag()) {
                    break;
                }
            }
            let _ = this.update(cx, |l, cx| {
                if l.open {
                    l.close(cx);
                }
            });
        })
        .detach();
    }

    // ---- Clipboard history

    fn clip_load(&mut self, cx: &mut Context<Self>) {
        let mut listed = clip::list(clip::thumb_dir());
        self.clip._listing = Some(cx.spawn(async move |this, cx| {
            while let Some(item) = listed.next().await {
                let alive = this.update(cx, |l, cx| {
                    match item {
                        Listed::Entries(entries) => l.clip.entries = entries,
                        Listed::Thumb(id) => {
                            l.clip.thumbs.insert(id);
                        }
                    }
                    l.changed(cx);
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    /// Makes sure the preview pane has what it needs for entry `id`.
    fn preview(&mut self, id: String, cx: &mut Context<Self>) {
        if self.clip.texts.contains_key(&id) {
            return;
        }
        self.clip.wanted = Some(id);
        self.next_preview(cx);
    }

    fn next_preview(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.clip.wanted.clone() else {
            return;
        };
        if self.clip.previewing || self.clip.texts.contains_key(&id) {
            return;
        }
        self.clip.previewing = true;
        let fetch = id.clone();
        runtime::then(
            cx,
            clip::preview(fetch, clip::thumb_dir()),
            move |l, preview, cx| {
                l.clip.previewing = false;
                l.clip
                    .texts
                    .insert(id.clone(), preview.text.unwrap_or_default());
                if let Some(meta) = preview.meta {
                    l.clip.metas.insert(id.clone(), meta);
                }
                if preview.thumb {
                    l.clip.thumbs.insert(id);
                }
                l.next_preview(cx);
                cx.notify();
            },
        );
    }

    /// "Text · 27 characters", "Code · 5 lines", "PNG · 1920 × 1080 · 2 MiB".
    pub fn clip_meta<'a>(&'a self, entry: &'a Entry) -> &'a str {
        self.clip.metas.get(&entry.id).unwrap_or(&entry.meta)
    }

    pub fn clip_remove(&mut self, entry: Entry, cx: &mut Context<Self>) {
        runtime::then(cx, clip::delete(entry.line), |l, (), cx| {
            if l.mode == Mode::Clip {
                l.clip_load(cx);
            }
        });
    }

    pub fn clip_wipe(&mut self, cx: &mut Context<Self>) {
        runtime::then(cx, clip::wipe(), |l, (), cx| {
            if l.mode == Mode::Clip {
                l.clip_load(cx);
            }
        });
    }

    // ---- 1Password

    fn pw_refresh(&mut self, cx: &mut Context<Self>) {
        let pw = &mut self.pw;
        pw.item = None;
        pw.error = None;
        pw.fields.clear();
        pw.want = None;
        pw.loading = pw.items.is_empty();
        pw.authorizing = false;
        if pw.loading {
            pw._slow_load = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SLOW_LOAD).await;
                let _ = this.update(cx, |l, cx| {
                    l.pw.authorizing = l.pw.loading;
                    l.changed(cx);
                });
            }));
        }
        if pw.listing {
            return;
        }
        pw.listing = true;
        runtime::then(cx, passwords::list(), |l, (items, error), cx| {
            let pw = &mut l.pw;
            pw.listing = false;
            pw.loading = false;
            pw.authorizing = false;
            pw._slow_load = None;
            pw.items = items;
            pw.ranked.clear();
            if error.is_some() {
                pw.error = error;
            }
            l.set_lists();
            l.ask(cx);
            l.next_fields(cx);
            l.changed(cx);
        });
    }

    /// Fetches an item's field metadata in the background (once per open).
    fn prefetch(&mut self, item: Item, cx: &mut Context<Self>) {
        if self.pw.fields.contains_key(&item.id) {
            return;
        }
        self.pw.want = Some(item);
        self.next_fields(cx);
    }

    fn next_fields(&mut self, cx: &mut Context<Self>) {
        let pw = &mut self.pw;
        let Some(item) = pw.want.clone() else {
            return;
        };
        if pw.fetching || pw.loading || pw.fields.contains_key(&item.id) {
            return;
        }
        pw.fetching = true;
        runtime::then(
            cx,
            passwords::fields(item.vault.clone(), item.id.clone()),
            move |l, (fields, error), cx| {
                l.pw.fetching = false;
                l.pw.fields.insert(item.id, fields);
                if error.is_some() {
                    l.pw.error = error;
                }
                l.next_fields(cx);
                l.changed(cx);
            },
        );
    }

    pub fn show_fields(&mut self, item: Item, cx: &mut Context<Self>) {
        self.pw.list_query = self.query.clone();
        self.pw.list_sel = self.sel;
        self.pw.item = Some(item.clone());
        self.prefetch(item, cx);
        self.set_text(String::new());
        self.sel = 0;
        self.changed(cx);
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.pw.item = None;
        self.set_text(self.pw.list_query.clone());
        self.sel = self.pw.list_sel;
        self.ask(cx);
        self.changed(cx);
    }

    /// `what`: a field id, "otp", or "password" / "username".
    fn take_pass(&mut self, item: Item, what: &str, paste: bool, cx: &mut Context<Self>) {
        self.close(cx);
        self.pass_list.used(&item.id);
        runtime::detach(passwords::take(
            item.vault,
            item.id,
            what.to_string(),
            paste,
        ));
    }

    pub fn take_field(&mut self, field: Field, paste: bool, cx: &mut Context<Self>) {
        let Some(item) = self.pw.item.clone() else {
            return;
        };
        let what = if field.is_otp() {
            "otp".to_string()
        } else {
            field.id
        };
        self.take_pass(item, &what, paste, cx);
    }

    /// Opens an item's website.
    pub fn open_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.close(cx);
        let url = if url.contains("://") {
            url
        } else {
            format!("https://{url}")
        };
        runtime::detach(async move { spawn("uwsm-app", &["--", "xdg-open", &url]).await });
    }

    // ---- Keys and clicks

    pub fn rows_len(&self) -> usize {
        match self.mode {
            Mode::Apps => self.shown_apps.len(),
            Mode::Files => self.files.len(),
            Mode::Clip => self.clip.shown.len(),
            Mode::Pass if self.pw.item.is_some() => self.pw.shown_fields.len(),
            Mode::Pass => self.pw.ranked.len(),
            Mode::Run | Mode::Calc => 0,
        }
    }

    pub fn row(&self, i: usize) -> Option<Row<'_>> {
        Some(match self.mode {
            Mode::Apps => Row::App(&self.apps[*self.shown_apps.get(i)?]),
            Mode::Files => Row::File(self.files.get(i)?),
            Mode::Clip => Row::Clip(&self.clip.entries[*self.clip.shown.get(i)?]),
            Mode::Pass => match &self.pw.item {
                Some(item) => {
                    let fields = self.pw.fields_of(item)?;
                    Row::Field(&fields[*self.pw.shown_fields.get(i)?])
                }
                None => Row::Pass(&self.pw.items[*self.pw.ranked.get(i)?]),
            },
            Mode::Run | Mode::Calc => return None,
        })
    }

    pub fn current_index(&self) -> Option<usize> {
        let n = self.rows_len();
        (n > 0).then(|| self.sel.min(n - 1))
    }

    pub fn current(&self) -> Option<Row<'_>> {
        self.row(self.current_index()?)
    }

    pub fn move_by(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.rows_len();
        if n > 0 {
            let at = self.sel.min(n - 1) as isize + by;
            self.sel = at.clamp(0, n as isize - 1) as usize;
            self.changed(cx);
        }
    }

    pub fn hover(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.sel != i && !self.dragging {
            self.sel = i;
            self.changed(cx);
        }
    }

    /// Escape: out of an item's fields, or close.
    pub fn escape(&mut self, cx: &mut Context<Self>) {
        if self.mode == Mode::Pass && self.pw.item.is_some() {
            self.back(cx);
        } else {
            self.close(cx);
        }
    }

    /// Backspace on an empty field: out of an item's fields, or drops the mode chip. False
    /// when there's nothing to drop (the field handles it).
    pub fn backspace_empty(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.query.is_empty() || self.mode == Mode::Apps {
            return false;
        }
        if self.mode == Mode::Pass && self.pw.item.is_some() {
            self.back(cx);
        } else {
            self.set_mode(Mode::Apps, Scope::Home, cx);
            self.changed(cx);
        }
        true
    }

    /// Tab (or → at the end of the text) opens the selected item's fields; shift+tab (or ← at
    /// the start) goes back. False when it doesn't apply.
    pub fn fields_key(&mut self, into: bool, cx: &mut Context<Self>) -> bool {
        if self.mode != Mode::Pass {
            return false;
        }
        match (into, self.pw.item.is_some()) {
            (true, false) => {
                if let Some(Row::Pass(item)) = self.current() {
                    let item = item.clone();
                    self.show_fields(item, cx);
                }
                true
            }
            (false, true) => {
                self.back(cx);
                true
            }
            _ => false,
        }
    }

    /// Delete in the clipboard history removes the selected entry.
    pub fn delete_key(&mut self, cx: &mut Context<Self>) -> bool {
        if self.mode != Mode::Clip {
            return false;
        }
        if let Some(Row::Clip(entry)) = self.current() {
            let entry = entry.clone();
            self.clip_remove(entry, cx);
        }
        true
    }

    /// Enter on the selection (or row `index`, clicked), with `mods`.
    pub fn activate(&mut self, index: Option<usize>, mods: Mods, cx: &mut Context<Self>) {
        let index = index.or(self.current_index());
        match self.mode {
            Mode::Apps => {
                if let Some(Row::App(app)) = index.and_then(|i| self.row(i)) {
                    let id = app.id.clone();
                    self.launch_app(id, cx);
                }
            }
            Mode::Run => {
                let cmd = self.query.trim().to_string();
                if cmd.is_empty() {
                    return;
                }
                self.close(cx);
                runtime::detach(async move { spawn("uwsm-app", &["--", "sh", "-c", &cmd]).await });
            }
            Mode::Calc => {
                // Shift opens it in Qalculate (history, plots, a window that stays), even
                // unfinished.
                let expr = self.query.trim().to_string();
                if mods.shift {
                    self.close(cx);
                    runtime::detach(async move {
                        let mut args = vec!["--", "qalculate-qt"];
                        if !expr.is_empty() {
                            args.extend(["--", expr.as_str()]);
                        }
                        spawn("uwsm-app", &args).await;
                    });
                    return;
                }
                let Some(calc) = self.calc.as_ref().filter(|c| c.ok) else {
                    return;
                };
                // The decimal value; ctrl for the exact form (2π, √(2), 1/3), alt as LaTeX.
                let text = if mods.alt {
                    calc.tex.clone()
                } else if mods.ctrl {
                    calc.exact.clone()
                } else {
                    calc.value.clone()
                };
                self.close(cx);
                copy(text);
            }
            Mode::Files => {
                if let Some(Row::File(file)) = index.and_then(|i| self.row(i)) {
                    let file = file.clone();
                    self.act_file(file, mods, cx);
                }
            }
            Mode::Clip => {
                if let Some(Row::Clip(entry)) = index.and_then(|i| self.row(i)) {
                    let id = entry.id.clone();
                    self.close(cx);
                    runtime::detach(clip::pick(id, !mods.shift));
                }
            }
            Mode::Pass => match index.and_then(|i| self.row(i)) {
                Some(Row::Field(field)) => {
                    let field = field.clone();
                    self.take_field(field, !mods.shift, cx);
                }
                Some(Row::Pass(item)) => {
                    let item = item.clone();
                    let what = if mods.ctrl {
                        "username"
                    } else if mods.alt {
                        "otp"
                    } else {
                        "password"
                    };
                    self.take_pass(item, what, !mods.shift, cx);
                }
                _ => {}
            },
        }
    }

    pub fn launch_app(&mut self, id: String, cx: &mut Context<Self>) {
        self.close(cx);
        self.apps_list.used(&id);
        let file = if id.ends_with(".desktop") {
            id
        } else {
            format!("{id}.desktop")
        };
        runtime::detach(async move { spawn("uwsm-app", &["--", &file]).await });
    }

    // ---- For the view

    pub fn show_recent(&self) -> bool {
        self.mode == Mode::Apps && self.query.trim().is_empty() && !self.recent.is_empty()
    }

    pub fn mode_label(&self) -> String {
        match self.mode {
            Mode::Files => format!(
                "Files · {}",
                match self.scope {
                    Scope::Home => "Home",
                    Scope::Downloads => "Downloads",
                    Scope::Root => "/",
                }
            ),
            Mode::Pass if self.pw.item.is_some() => "Passwords · Fields".into(),
            Mode::Run => "Run".into(),
            Mode::Calc => "Calculate".into(),
            Mode::Clip => "Clipboard".into(),
            Mode::Pass => "Passwords".into(),
            Mode::Apps => String::new(),
        }
    }

    pub fn placeholder(&self) -> &'static str {
        match self.mode {
            Mode::Apps => "Search apps",
            Mode::Run => "Command",
            Mode::Calc => "Expression",
            Mode::Files => match self.scope {
                Scope::Root => "Search the whole system",
                Scope::Downloads => "Search Downloads",
                Scope::Home => "Search files",
            },
            Mode::Clip => "Search clipboard",
            Mode::Pass if self.pw.item.is_some() => "Filter fields",
            Mode::Pass => "Search 1Password",
        }
    }

    pub fn files_heading(&self) -> &'static str {
        let q = !self.query.trim().is_empty();
        match (self.scope, q) {
            (Scope::Downloads, true) => "In Downloads",
            (Scope::Downloads, false) => "Downloads · newest first",
            (Scope::Root, true) => "Everywhere",
            (Scope::Root, false) => "Type to search the whole system",
            (Scope::Home, true) => "In Home",
            (Scope::Home, false) => "Recent files",
        }
    }

    pub fn hints(&self) -> Vec<(&'static [&'static str], &'static str)> {
        match self.mode {
            Mode::Files => vec![
                (&["↵"], "Open"),
                (&["⇧", "↵"], "Show in Files"),
                (&["ctrl", "↵"], "Copy path"),
                (&["alt", "↵"], "Shelf"),
            ],
            Mode::Clip => vec![
                (&["↵"], "Paste"),
                (&["⇧", "↵"], "Copy"),
                (&["del"], "Remove"),
            ],
            Mode::Pass if self.pw.item.is_some() => {
                vec![(&["↵"], "Paste"), (&["⇧", "↵"], "Copy"), (&["esc"], "Back")]
            }
            Mode::Pass => vec![
                (&["↵"], "Password"),
                (&["ctrl", "↵"], "Username"),
                (&["alt", "↵"], "Code"),
                (&["⇥"], "Fields"),
            ],
            Mode::Run => vec![(&["↵"], "Run"), (&["esc"], "Close")],
            Mode::Calc => vec![
                (&["↵"], "Copy value"),
                (&["ctrl", "↵"], "Exact"),
                (&["alt", "↵"], "LaTeX"),
                (&["⇧", "↵"], "Qalculate"),
            ],
            Mode::Apps => vec![
                (&["↑", "↓"], "Navigate"),
                (&["~"], "Files"),
                (&["\""], "Clipboard"),
                (&["*"], "Passwords"),
                (&[">"], "Run"),
                (&["="], "Calc"),
            ],
        }
    }

    /// The footer's count or state.
    pub fn status(&self) -> String {
        let n = self.rows_len();
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        match self.mode {
            Mode::Apps => plural(n, "app", "apps"),
            Mode::Run => "Shell command".into(),
            Mode::Calc => "Calculator".into(),
            Mode::Files => {
                if self.files_busy() && n == 0 {
                    return "Searching…".into();
                }
                let count = if self.files_total > n {
                    format!(
                        "{n} of {}{} files",
                        self.files_total,
                        if self.files_capped { "+" } else { "" }
                    )
                } else {
                    plural(n, "file", "files")
                };
                if self.scope == Scope::Root {
                    format!("{count} · skips /nix/store")
                } else {
                    count
                }
            }
            Mode::Clip => format!("{n} of {} entries", self.clip.entries.len()),
            Mode::Pass => {
                if self.pw.authorizing {
                    "Authorize in 1Password…".into()
                } else if self.pw.loading {
                    "Loading…".into()
                } else if let Some(error) = &self.pw.error {
                    error.clone()
                } else if let Some(item) = &self.pw.item {
                    format!("{n} fields · {}", item.title)
                } else {
                    format!("{} · 1Password", plural(n, "item", "items"))
                }
            }
        }
    }

    /// The footer's status reads as a warning.
    pub fn status_is_error(&self) -> bool {
        self.mode == Mode::Pass && self.pw.error.is_some() && !self.pw.loading
    }

    /// The empty list's text, if it shows one.
    pub fn empty_text(&self) -> Option<&'static str> {
        if self.rows_len() > 0 {
            return None;
        }
        let q = !self.query.trim().is_empty();
        match self.mode {
            Mode::Apps => Some("No matches"),
            Mode::Files if !self.files_busy() && (q || self.scope != Scope::Root) => {
                Some(match (q, self.scope) {
                    (false, Scope::Home) => "No recent files",
                    (false, _) => "Nothing here",
                    (true, _) => "No matches",
                })
            }
            Mode::Clip if self.clip.entries.is_empty() => Some("Clipboard history is empty"),
            Mode::Clip => Some("No matches"),
            Mode::Pass if self.pw.loading => None,
            Mode::Pass if self.pw.item.is_some() => Some("No fields"),
            Mode::Pass if self.pw.items.is_empty() => Some("No items"),
            Mode::Pass => Some("No matches"),
            _ => None,
        }
    }
}

/// Puts `text` on the clipboard.
fn copy(text: String) {
    runtime::detach(async move { spawn("wl-copy", &["--", &text]).await });
}

/// `path` as a file:// URL, each part percent-encoded as JavaScript's encodeURIComponent does.
fn file_url(path: &str) -> String {
    let encode = |part: &str| {
        let mut out = String::new();
        for b in part.bytes() {
            if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    };
    format!(
        "file://{}",
        path.split('/').map(encode).collect::<Vec<_>>().join("/")
    )
}

/// How long ago `mtime` (seconds) was, shortly: "now", "5 min", "3 h", "2 d", "1 w", "4 mo".
pub fn age(mtime: i64, now: i64) -> String {
    let s = now - mtime;
    match s {
        s if s < 60 => "now".into(),
        s if s < 3600 => format!("{} min", s / 60),
        s if s < 86_400 => format!("{} h", s / 3600),
        s if s < 86_400 * 7 => format!("{} d", s / 86_400),
        s if s < 86_400 * 30 => format!("{} w", s / 86_400 / 7),
        s if s < 86_400 * 365 => format!("{} mo", s / 86_400 / 30),
        s => format!("{} y", s / 86_400 / 365),
    }
}

/// A stable colour per name for letter tiles, as the QML's string hash picked it.
pub fn tint(name: &str) -> u32 {
    const PALETTE: [u32; 15] = [
        0x3b6fd6, 0xd9822b, 0x2f9e8f, 0x4a86c8, 0x7a6a55, 0x3d4b8f, 0x3f8f5a, 0x7b3f8c, 0x8a3f78,
        0x2c9b55, 0x2b4a6e, 0x5865c9, 0x3a6ea8, 0x6d4aff, 0xb0533f,
    ];
    let mut h: i32 = 0;
    for c in name.chars() {
        // charCodeAt(0) of each code point: its first UTF-16 unit.
        let mut units = [0u16; 2];
        let unit = c.encode_utf16(&mut units)[0];
        h = h.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    PALETTE[((h as i64).unsigned_abs() % PALETTE.len() as u64) as usize]
}

/// Anchored to the top edge only, so the compositor centres it; over everything, with the
/// keyboard.
fn layer() -> LayerShellOptions {
    LayerShellOptions {
        namespace: "shell:launcher".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP,
        exclusive_zone: Some(px(-1.)),
        margin: Some((px(TOP), px(0.), px(0.), px(0.))),
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes() {
        assert!(matches!(
            split_prefix("=1+2"),
            Some((Mode::Calc, Scope::Home, "1+2"))
        ));
        assert!(matches!(
            split_prefix("d~report"),
            Some((Mode::Files, Scope::Downloads, "report"))
        ));
        assert!(matches!(
            split_prefix("/etc"),
            Some((Mode::Files, Scope::Root, "etc"))
        ));
        assert!(matches!(split_prefix("\"x"), Some((Mode::Clip, _, "x"))));
        assert!(matches!(split_prefix("*"), Some((Mode::Pass, _, ""))));
        assert!(matches!(split_prefix(">ls"), Some((Mode::Run, _, "ls"))));
        assert!(split_prefix("firefox").is_none());
        assert!(split_prefix("").is_none());
        assert!(split_prefix("d").is_none());
    }

    #[test]
    fn ages() {
        assert_eq!(age(100, 130), "now");
        assert_eq!(age(0, 300), "5 min");
        assert_eq!(age(0, 3 * 3600), "3 h");
        assert_eq!(age(0, 2 * 86_400), "2 d");
        assert_eq!(age(0, 14 * 86_400), "2 w");
        assert_eq!(age(0, 90 * 86_400), "3 mo");
        assert_eq!(age(0, 800 * 86_400), "2 y");
    }

    #[test]
    fn urls() {
        assert_eq!(
            file_url("/home/u/a b/ö.txt"),
            "file:///home/u/a%20b/%C3%B6.txt"
        );
        assert_eq!(file_url("/x/(1)!.md"), "file:///x/(1)!.md");
    }

    #[test]
    fn tints() {
        // As JavaScript computes it: h = (h * 31 + c) | 0, then |h| % 15.
        let js = |s: &str| {
            let mut h: i64 = 0;
            for u in s.encode_utf16() {
                h = ((h * 31 + i64::from(u)) as i32) as i64;
            }
            (h.abs() % 15) as usize
        };
        for name in [
            "Firefox",
            "",
            "a",
            "Zed Editor with a very long name indeed",
            "Ålänning",
        ] {
            let palette_index = [
                0x3b6fd6, 0xd9822b, 0x2f9e8f, 0x4a86c8, 0x7a6a55, 0x3d4b8f, 0x3f8f5a, 0x7b3f8c,
                0x8a3f78, 0x2c9b55, 0x2b4a6e, 0x5865c9, 0x3a6ea8, 0x6d4aff, 0xb0533f,
            ]
            .iter()
            .position(|&c| c == tint(name))
            .unwrap();
            assert_eq!(palette_index, js(name), "{name}");
        }
    }
}
