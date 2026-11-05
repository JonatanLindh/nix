//! The launcher's window (the card in Launcher.qml): the search row with its mode chip, the
//! current mode's list or result, and a footer with the count and the keys. Only the card takes
//! input and is blurred behind; the rest of the window is see-through and click-through.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::component::input::{
    Backspace, Delete, Enter, Escape, IndentInline, Input, InputEvent, InputState, MoveDown,
    MoveLeft, MovePageDown, MovePageUp, MoveRight, MoveUp, OutdentInline,
};
use gpui::{
    AnyElement, App, Bounds, Context, Div, Entity, ExternalDragPayload, FileDragPaths, Pixels,
    Rgba, ScrollStrategy, SharedString, Stateful, UniformListScrollHandle, Window, canvas, div,
    img, prelude::*, px, rgb, uniform_list,
};

use super::{Launcher, Mode, Mods, Row, age, formula, tint};
use crate::services::icons::Image;
use crate::services::launcher::clip;
use crate::services::launcher::passwords::Item;
use crate::ui::controls::{pill, section};
use crate::ui::{hstack, icon, key_cap, label, small, vstack};
use crate::{theme, wayland};

const CARD_RADIUS: f32 = 24.;
/// Rows and the 2px between them, as the lists' spacing.
const ROW: f32 = 44.;
const TALL_ROW: f32 = 48.;
const SPACING: f32 = 2.;
const SPLIT_HEIGHT: f32 = 400.;
const LEFT_WIDTH: f32 = 290.;

/// The card's bounds, and whether it's hidden for a drag.
type Region = (Bounds<Pixels>, bool);

pub struct View {
    model: Entity<Launcher>,
    input: Entity<InputState>,
    scroll: UniformListScrollHandle,
    /// The selection last scrolled to.
    followed: Option<(Mode, usize)>,
    placeholder: &'static str,
    /// The card's bounds as last given to the compositor (input and blur), and whether it was
    /// hidden for a drag.
    region: Rc<RefCell<Option<Region>>>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl View {
    pub fn new(model: Entity<Launcher>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = model.read(cx).placeholder();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        input.update(cx, |i, cx| i.focus(window, cx));
        let subscriptions = vec![
            cx.subscribe_in(&input, window, |this, input, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    let text = input.read(cx).value().to_string();
                    this.model.update(cx, |l, cx| l.text_edited(text, cx));
                }
            }),
            cx.observe_in(&model, window, |this, _, window, cx| this.sync(window, cx)),
        ];
        let mut view = Self {
            model,
            input,
            scroll: UniformListScrollHandle::new(),
            followed: None,
            placeholder,
            region: Rc::default(),
            _subscriptions: subscriptions,
        };
        view.sync(window, cx);
        view
    }

    /// Puts text set by the launcher (cleared, a prefix taken off) in the field, and the mode's
    /// placeholder.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (text, placeholder) = self
            .model
            .update(cx, |l, _| (l.take_text(), l.placeholder()));
        if let Some(text) = text {
            self.input.update(cx, |i, cx| i.set_value(text, window, cx));
        }
        if placeholder != self.placeholder {
            self.placeholder = placeholder;
            self.input
                .update(cx, |i, cx| i.set_placeholder(placeholder, window, cx));
        }
        cx.notify();
    }

    /// The search field's text length and cursor (bytes).
    fn cursor(input: &Entity<InputState>, cx: &App) -> (usize, usize) {
        let input = input.read(cx);
        (input.value().len(), input.cursor())
    }
}

impl Render for View {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.clone();
        let l = model.read(cx);

        // Keep the selection in view as it moves.
        if let Some(i) = l.current_index()
            && self.followed != Some((l.mode, i))
        {
            self.followed = Some((l.mode, i));
            self.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
        }

        let dragging = l.dragging;
        let mut body = vstack(0.).w_full().child(search_row(l, &self.input));
        body = body.child(crate::ui::divider());
        body = match l.mode {
            Mode::Clip | Mode::Pass => body.child(split(&model, l, &self.scroll)),
            _ => body.child(single(&model, l, &self.scroll, window, cx)),
        };
        body = body.child(crate::ui::divider()).child(footer(&model, l));

        let card = crate::ui::glass(CARD_RADIUS)
            .w_full()
            .overflow_hidden()
            .when(dragging, |c| c.opacity(0.))
            .child(body);

        // Only the card takes clicks and is blurred behind; nothing while a file is dragged out.
        let region = self.region.clone();
        let fit = canvas(
            move |bounds, window, _| {
                if *region.borrow() != Some((bounds, dragging)) {
                    if dragging {
                        window.set_input_region(Some(&[]));
                        wayland::set_blur(window, &[]);
                    } else {
                        window.set_input_region(Some(&[bounds]));
                        wayland::set_blur(window, &[(bounds, CARD_RADIUS)]);
                    }
                    *region.borrow_mut() = Some((bounds, dragging));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        keys(crate::ui::root(), &model, &self.input)
            .child(div().relative().flex_none().w_full().child(card).child(fit))
    }
}

/// The keys the launcher takes before the search field does.
fn keys(root: Div, model: &Entity<Launcher>, input: &Entity<InputState>) -> Div {
    let m = model.clone();
    let on = move |f: fn(&mut Launcher, &mut Context<Launcher>)| {
        let m = m.clone();
        move |window: &mut Window, cx: &mut App| {
            let _ = window;
            m.update(cx, f);
            cx.stop_propagation();
        }
    };
    let (up, down, page_up, page_down, escape) = (
        on(|l, cx| l.move_by(-1, cx)),
        on(|l, cx| l.move_by(1, cx)),
        on(|l, cx| l.move_by(-8, cx)),
        on(|l, cx| l.move_by(8, cx)),
        on(|l, cx| l.escape(cx)),
    );
    let (enter, alt_enter) = (model.clone(), model.clone());
    let (tab, back_tab, left, right, backspace, delete) = (
        model.clone(),
        model.clone(),
        model.clone(),
        model.clone(),
        model.clone(),
        model.clone(),
    );
    let (left_input, right_input, delete_input) = (input.clone(), input.clone(), input.clone());
    root.capture_action(move |_: &MoveUp, w, cx| up(w, cx))
        .capture_action(move |_: &MoveDown, w, cx| down(w, cx))
        .capture_action(move |_: &MovePageUp, w, cx| page_up(w, cx))
        .capture_action(move |_: &MovePageDown, w, cx| page_down(w, cx))
        .capture_action(move |_: &Escape, w, cx| escape(w, cx))
        .capture_action(move |_: &Enter, window, cx| {
            let mods = Mods::from(window.modifiers());
            enter.update(cx, |l, cx| l.activate(None, mods, cx));
            cx.stop_propagation();
        })
        // Alt+enter isn't one of the field's keys, so it comes as a plain key press.
        .capture_key_down(move |event, _, cx| {
            if event.keystroke.key == "enter" {
                let mods = Mods::from(event.keystroke.modifiers);
                alt_enter.update(cx, |l, cx| l.activate(None, mods, cx));
                cx.stop_propagation();
            }
        })
        // Tab never types into the field.
        .capture_action(move |_: &IndentInline, _, cx| {
            tab.update(cx, |l, cx| l.fields_key(true, cx));
            cx.stop_propagation();
        })
        .capture_action(move |_: &OutdentInline, _, cx| {
            back_tab.update(cx, |l, cx| l.fields_key(false, cx));
            cx.stop_propagation();
        })
        // ← at the start of the text and → at its end move between an item and its fields.
        .capture_action(move |_: &MoveLeft, _, cx| {
            let (_, cursor) = View::cursor(&left_input, cx);
            if cursor == 0
                && left.read(cx).pw.item.is_some()
                && left.update(cx, |l, cx| l.fields_key(false, cx))
            {
                cx.stop_propagation();
            }
        })
        .capture_action(move |_: &MoveRight, _, cx| {
            let (len, cursor) = View::cursor(&right_input, cx);
            if cursor == len
                && right.read(cx).pw.item.is_none()
                && right.update(cx, |l, cx| l.fields_key(true, cx))
            {
                cx.stop_propagation();
            }
        })
        .capture_action(move |_: &Backspace, _, cx| {
            if backspace.update(cx, |l, cx| l.backspace_empty(cx)) {
                cx.stop_propagation();
            }
        })
        // Delete (at the end of the text, or with shift) removes a clipboard entry.
        .capture_action(move |_: &Delete, window, cx| {
            let (len, cursor) = View::cursor(&delete_input, cx);
            if (window.modifiers().shift || cursor == len)
                && delete.update(cx, |l, cx| l.delete_key(cx))
            {
                cx.stop_propagation();
            }
        })
}

// ---- Pieces

/// "[ctrl][↵] Copy path": key caps, then what they do.
fn key_hint(keys: &[&str], text: &str) -> Div {
    hstack(5.)
        .flex_none()
        .children(keys.iter().map(|k| {
            key_cap(
                SharedString::from(k.to_string()),
                theme::TEXT,
                theme::alpha(theme::WHITE, 0.1),
            )
            .min_w(px(18.))
            .justify_center()
            .border_1()
            .border_color(theme::alpha(theme::WHITE, 0.1))
        }))
        .when(!text.is_empty(), |row| {
            row.child(small(SharedString::from(text.to_string()), theme::TEXT_DIM))
        })
}

fn color(hex: u32) -> Rgba {
    rgb(hex)
}

/// A tile with the first letter of `name`, for apps without an icon and 1Password items.
fn letter_tile(name: &str, size: f32, radius: f32) -> Div {
    let letter: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    div()
        .flex_none()
        .relative()
        .size(px(size))
        .rounded(px(radius))
        .bg(color(tint(name)))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .absolute()
                .top(px(1.))
                .left(px(radius))
                .w(px((size - 2. * radius).max(0.)))
                .h(px(1.))
                .bg(theme::alpha(theme::WHITE, 0.25)),
        )
        .child(
            div()
                .text_color(gpui::white())
                .text_size(px((size * 0.42).round()))
                .font_weight(theme::SEMIBOLD)
                .child(letter),
        )
}

/// A tinted square with a line icon (file types, clipboard kinds, the run row).
fn icon_tile(name: &str, bg: Rgba, size: f32, icon_size: f32, ink: Rgba) -> Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(8.))
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(icon(name, icon_size, ink))
}

/// An app's icon from its desktop entry, or a letter tile when the theme has none.
fn app_icon(l: &Launcher, app: &super::apps::App, size: f32) -> AnyElement {
    let image = app
        .icon
        .as_ref()
        .and_then(|n| l.icons.get(n))
        .and_then(Option::as_ref);
    match image {
        Some(Image::Svg(path)) => img(path.clone())
            .flex_none()
            .size(px(size))
            .into_any_element(),
        Some(Image::Raster(image)) => img(image.clone())
            .flex_none()
            .size(px(size))
            .into_any_element(),
        None => letter_tile(&app.name, size, (size * 0.27).round()).into_any_element(),
    }
}

/// A list row: highlighted while selected, hovering selects it.
fn row(
    model: &Entity<Launcher>,
    id: impl Into<SharedString>,
    i: usize,
    selected: bool,
    height: f32,
) -> Stateful<Div> {
    let hover = model.clone();
    div()
        .id(gpui::ElementId::Name(id.into()))
        .group("row")
        .relative()
        .flex_none()
        .w_full()
        .h(px(height))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(12.))
        .rounded(px(12.))
        .cursor_pointer()
        .when(selected, |r| r.bg(theme::FILL_HOVER))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                hover.update(cx, |l, cx| l.hover(i, cx));
            }
        })
}

/// Clicking the row with a modifier does what enter with it does.
fn on_activate(row: Stateful<Div>, model: &Entity<Launcher>, i: usize) -> Stateful<Div> {
    let model = model.clone();
    row.on_click(move |event, _, cx| {
        let mods = Mods::from(event.modifiers());
        model.update(cx, |l, cx| l.activate(Some(i), mods, cx));
    })
}

/// A row with the gap after it, as uniform lists need one height per item.
fn spaced(row: impl IntoElement, height: f32) -> Div {
    div()
        .w_full()
        .h(px(height + SPACING))
        .pb(px(SPACING))
        .child(row)
}

fn empty(text: &'static str) -> Div {
    div()
        .w_full()
        .py(px(22.))
        .flex()
        .justify_center()
        .child(label(text, theme::TEXT_DIM))
}

/// A list of `n` rows of `height`, at most `max` tall.
fn list(
    id: &'static str,
    n: usize,
    height: f32,
    max: Option<f32>,
    scroll: &UniformListScrollHandle,
    item: impl Fn(usize, &Launcher, &mut Window) -> AnyElement + 'static,
    model: &Entity<Launcher>,
) -> AnyElement {
    let m = model.clone();
    let list = uniform_list(id, n, move |range, window, cx| {
        let l = m.read(cx);
        range
            .map(|i| spaced(item(i, l, window), height).into_any_element())
            .collect::<Vec<_>>()
    })
    .track_scroll(scroll)
    .w_full();
    match max {
        Some(max) => list
            .h(px((n as f32 * (height + SPACING)).min(max)))
            .into_any_element(),
        None => list.h_full().into_any_element(),
    }
}

// ---- The search row

fn search_row(l: &Launcher, input: &Entity<InputState>) -> Div {
    let mono = matches!(l.mode, Mode::Run | Mode::Calc);
    hstack(12.)
        .w_full()
        .h(px(62.))
        .px(px(20.))
        .child(icon("search", 20., theme::alpha(theme::TEXT, 0.7)))
        .when(l.mode != Mode::Apps, |row| {
            row.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .h(px(26.))
                    .px(px(10.))
                    .rounded(px(8.))
                    .bg(theme::ACCENT)
                    .text_color(theme::INK)
                    .text_size(px(theme::SMALL))
                    .font_weight(theme::SEMIBOLD)
                    .child(l.mode_label()),
            )
        })
        .child(
            div().flex_1().min_w_0().child(
                Input::new(input)
                    .appearance(false)
                    .h(px(30.))
                    .px_0()
                    .text_size(px(20.))
                    .font_family(if mono { theme::MONO } else { theme::FONT })
                    .text_color(theme::WHITE),
            ),
        )
}

// ---- Single-column modes: apps, run, calc, files

fn single(
    model: &Entity<Launcher>,
    l: &Launcher,
    scroll: &UniformListScrollHandle,
    window: &mut Window,
    cx: &App,
) -> Div {
    let mut column = vstack(6.).w_full().p(px(10.));
    match l.mode {
        Mode::Apps => {
            let recent = l.show_recent();
            if recent {
                column = column
                    .child(section("Recent"))
                    .child(recent_row(model, l))
                    .child(section("Applications"));
            }
            let n = l.shown_apps.len();
            if n > 0 {
                let m = model.clone();
                column = column.child(list(
                    "apps",
                    n,
                    ROW,
                    Some(if recent { 226. } else { 440. }),
                    scroll,
                    move |i, l, _| app_row(&m, l, i),
                    model,
                ));
            }
        }
        Mode::Run => column = column.child(run_row(l)),
        Mode::Calc => column = column.child(calc_box(l, window)),
        Mode::Files => {
            column = column.child(section(l.files_heading()).pb(px(2.)));
            let n = l.files.len();
            if n > 0 {
                let m = model.clone();
                let on_shelf: Vec<bool> = l.files.iter().map(|f| l.on_shelf(&f.path, cx)).collect();
                column = column.child(list(
                    "files",
                    n,
                    TALL_ROW,
                    Some(440.),
                    scroll,
                    move |i, l, _| file_row(&m, l, i, on_shelf.get(i).copied().unwrap_or(false)),
                    model,
                ));
            }
        }
        Mode::Clip | Mode::Pass => {}
    }
    if let Some(text) = l.empty_text() {
        column = column.child(empty(text));
    }
    column
}

/// The most used apps as big tiles, while the search is empty.
fn recent_row(model: &Entity<Launcher>, l: &Launcher) -> Div {
    let tiles = l.recent.iter().take(6).map(|&i| {
        let app = &l.apps[i];
        let launch = model.clone();
        let id = app.id.clone();
        div()
            .id(SharedString::from(format!("recent-{}", app.id)))
            .flex_1()
            .min_w_0()
            .h(px(92.))
            .rounded(px(14.))
            .cursor_pointer()
            .hover(|s| s.bg(theme::alpha(theme::WHITE, 0.09)))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(10.))
            .px(px(4.))
            .gap(px(6.))
            .child(app_icon(l, app, 48.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .text_size(px(theme::SMALL))
                    .text_color(theme::alpha(theme::TEXT, 0.85))
                    .child(div().truncate().child(app.name.clone())),
            )
            .on_click(move |_, _, cx| {
                let id = id.clone();
                launch.update(cx, |l, cx| l.launch_app(id, cx));
            })
            .into_any_element()
    });
    let fill = (l.recent.len().min(6)..6).map(|_| div().flex_1().into_any_element());
    hstack(4.)
        .w_full()
        .pb(px(4.))
        .children(tiles)
        .children(fill)
}

fn app_row(model: &Entity<Launcher>, l: &Launcher, i: usize) -> AnyElement {
    let Some(Row::App(app)) = l.row(i) else {
        return div().into_any_element();
    };
    let selected = l.current_index() == Some(i);
    let row = row(model, format!("app-{}", app.id), i, selected, ROW)
        .child(app_icon(l, app, 30.))
        .child(
            div()
                .flex_none()
                .max_w(px(330.))
                .truncate()
                .font_weight(theme::WEIGHT)
                .child(app.name.clone()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(theme::alpha(theme::TEXT, 0.62))
                .child(app.detail.clone()),
        )
        .when(selected, |r| r.child(key_hint(&["↵"], "Open")));
    on_activate(row, model, i).into_any_element()
}

fn run_row(l: &Launcher) -> Div {
    let cmd = l.query.trim();
    hstack(12.)
        .w_full()
        .h(px(ROW))
        .px(px(10.))
        .rounded(px(12.))
        .bg(theme::FILL_HOVER)
        .child(icon_tile(
            "terminal",
            color(0x2b3a3e),
            30.,
            16.,
            theme::TEXT,
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(theme::MONO)
                .text_color(if cmd.is_empty() {
                    theme::TEXT_FAINT
                } else {
                    theme::TEXT
                })
                .child(if cmd.is_empty() {
                    "Type a command".to_string()
                } else {
                    cmd.to_string()
                }),
        )
        .child(key_hint(&["↵"], "Run in shell"))
}

/// The calculator's result, big: typeset when qalc's LaTeX could be read, as text otherwise,
/// with the decimal value under an exact one.
fn calc_box(l: &Launcher, window: &mut Window) -> Div {
    let calc = l.calc.as_ref();
    let ok = calc.is_some_and(|c| c.ok);
    let query = l.query.trim();
    // The card's width less the margins and the hint beside the result.
    let room = 720. - 2. * 10. - 2. * 4. - 20. - 16. - 20. - 70.;
    let result: AnyElement = match (&l.math, calc) {
        (Some(math), _) => formula::view(math, 34., theme::WHITE, room, window),
        (None, Some(calc)) => div()
            .w_full()
            .font_family(theme::MONO)
            .text_size(px(32.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(if calc.ok {
                theme::WHITE
            } else {
                theme::alpha(theme::TEXT, 0.5)
            })
            .child(calc.text.clone())
            .into_any_element(),
        (None, None) => div().h(px(40.)).into_any_element(),
    };
    let approx = calc.map(|c| c.approx.as_str()).unwrap_or_default();
    // Equations come with their own: "x ≈ 1,414213562 or x ≈ −1,414213562".
    let approx = if approx.is_empty() {
        None
    } else if approx.contains('≈') {
        Some(approx.to_string())
    } else {
        Some(format!("≈ {approx}"))
    };
    let mono_dim = |text: String| {
        div()
            .w_full()
            .font_family(theme::MONO)
            .text_color(theme::TEXT_DIM)
            .child(text)
    };
    hstack(16.)
        .m(px(4.))
        .py(px(18.))
        .pl(px(20.))
        .pr(px(20.))
        .rounded(px(16.))
        .bg(theme::FILL)
        .child(
            vstack(4.)
                .flex_1()
                .min_w_0()
                .child(mono_dim(if query.is_empty() {
                    " ".into()
                } else {
                    query.to_string()
                }))
                .child(result)
                .children(approx.map(mono_dim)),
        )
        .child(key_hint(&["↵"], "Copy").when(!ok, |h| h.opacity(0.4)))
}

#[derive(Clone)]
struct Dragged {
    path: PathBuf,
    name: SharedString,
}

/// The chip under the pointer while a file is dragged inside the window.
struct DragChip(SharedString);

impl Render for DragChip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(5.))
            .rounded(px(14.))
            .bg(theme::WHITE)
            .text_color(theme::INK)
            .text_size(px(theme::SMALL))
            .font_family(theme::FONT)
            .child(self.0.clone())
    }
}

fn file_tint(kind: &str) -> Rgba {
    color(match kind {
        "folder" => 0x3f78b8,
        "image" => 0x8c5cb0,
        "code" => 0x2f8f7f,
        "archive" => 0x7a6a4a,
        "video" => 0xb0753f,
        _ => 0xb0533f,
    })
}

/// Shortens `text` in the middle to about `max` characters.
fn elide_middle(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max {
        return text.to_string();
    }
    let head = (max - 1) / 2;
    let tail = max - 1 - head;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// A file: click opens (with the modifiers enter takes), dragging drags it out.
fn file_row(model: &Entity<Launcher>, l: &Launcher, i: usize, shelved: bool) -> AnyElement {
    let Some(Row::File(file)) = l.row(i) else {
        return div().into_any_element();
    };
    let selected = l.current_index() == Some(i);
    let now = chrono::Local::now().timestamp();
    let kind = file.kind;
    let mut row = row(model, format!("file-{}", file.path), i, selected, TALL_ROW)
        .child(icon_tile(kind, file_tint(kind), 30., 16., theme::TEXT))
        .child(
            vstack(2.)
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .font_weight(theme::WEIGHT)
                        .child(file.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(theme::MONO)
                        .text_size(px(11.))
                        .text_color(theme::alpha(theme::TEXT, 0.6))
                        .child(elide_middle(&file.dir, 70)),
                ),
        )
        .when(shelved, |r| {
            r.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .h(px(20.))
                    .px(px(8.))
                    .rounded(px(10.))
                    .bg(theme::alpha(theme::ACCENT, 0.26))
                    .text_size(px(11.))
                    .child("On shelf"),
            )
        });
    row = if selected {
        row.child(icon("grip", 14., theme::alpha(theme::TEXT, 0.5)))
            .child(key_hint(&["↵"], "Open"))
    } else {
        row.child(
            div()
                .flex_none()
                .text_size(px(theme::SMALL))
                .text_color(theme::alpha(theme::TEXT, 0.6))
                .child(age(file.mtime, now)),
        )
    };
    let dragged = Dragged {
        path: PathBuf::from(&file.path),
        name: file.name.clone().into(),
    };
    let (start, out) = (model.clone(), model.clone());
    on_activate(row, model, i)
        .on_drag(dragged, move |d: &Dragged, _, _, cx| {
            start.update(cx, |l, cx| l.drag_started(cx));
            let name = d.name.clone();
            cx.new(|_| DragChip(name))
        })
        .external_drag_payload::<Dragged>(move |d, _, cx| {
            out.update(cx, |l, cx| l.drag_out(cx));
            let entries = [(d.path.clone(), d.path.is_dir())];
            Some(ExternalDragPayload::Files(FileDragPaths::new(entries)))
        })
        .into_any_element()
}

// ---- Split modes: clipboard and passwords, the list on the left and detail on the right

fn split(model: &Entity<Launcher>, l: &Launcher, scroll: &UniformListScrollHandle) -> Div {
    let n = l.rows_len();
    let m = model.clone();
    let mut left = div()
        .flex_none()
        .w(px(LEFT_WIDTH))
        .h_full()
        .relative()
        .border_r_1()
        .border_color(theme::DIVIDER)
        .pt(px(10.))
        .pb(px(10.))
        .pl(px(10.))
        .pr(px(6.));
    if n > 0 {
        left = left.child(if l.mode == Mode::Clip {
            list(
                "clip",
                n,
                TALL_ROW,
                None,
                scroll,
                move |i, l, _| clip_row(&m, l, i),
                model,
            )
        } else {
            list(
                "pass",
                n,
                TALL_ROW,
                None,
                scroll,
                move |i, l, _| pass_row(&m, l, i),
                model,
            )
        });
    }
    if let Some(text) = l.empty_text() {
        left = left.child(empty(text));
    }

    let right: AnyElement = match l.mode {
        Mode::Clip => match l.current() {
            Some(Row::Clip(entry)) => clip_preview(l, entry).into_any_element(),
            _ => div().flex_1().into_any_element(),
        },
        _ => {
            let item = l.pw.item.as_ref().or(match l.current() {
                Some(Row::Pass(item)) => Some(item),
                _ => None,
            });
            match item {
                Some(item) => pass_detail(model, l, item).into_any_element(),
                None => div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(l.pw.authorizing, |d| {
                        d.child(label(
                            "Waiting for 1Password to authorize…",
                            theme::TEXT_DIM,
                        ))
                    })
                    .into_any_element(),
            }
        }
    };
    div()
        .w_full()
        .h(px(SPLIT_HEIGHT))
        .flex()
        .flex_row()
        .child(left)
        .child(right)
}

fn clip_row(model: &Entity<Launcher>, l: &Launcher, i: usize) -> AnyElement {
    let Some(Row::Clip(entry)) = l.row(i) else {
        return div().into_any_element();
    };
    let selected = l.current_index() == Some(i);
    let kind = if matches!(entry.kind.as_str(), "text" | "link" | "code" | "image") {
        entry.kind.as_str()
    } else {
        "text"
    };
    let remove = model.clone();
    let removed = entry.clone();
    let row = row(model, format!("clip-{}", entry.id), i, selected, TALL_ROW)
        .child(icon_tile(
            kind,
            theme::alpha(theme::WHITE, 0.1),
            28.,
            15.,
            theme::alpha(theme::TEXT, 0.85),
        ))
        .child(
            vstack(2.)
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(entry.title.clone()))
                .child(
                    div()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(theme::alpha(theme::TEXT, 0.6))
                        .child(l.clip_meta(entry).to_string()),
                ),
        )
        .child(
            div()
                .id(SharedString::from(format!("remove-{}", entry.id)))
                .flex_none()
                .size(px(22.))
                .rounded(px(11.))
                .flex()
                .items_center()
                .justify_center()
                .invisible()
                .group_hover("row", |s| s.visible())
                .hover(|s| s.bg(theme::FILL_HOVER))
                .child(icon("close", 11., theme::TEXT))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    let entry = removed.clone();
                    remove.update(cx, |l, cx| l.clip_remove(entry, cx));
                }),
        );
    // Click pastes like enter; shift+click only copies.
    on_activate(row, model, i).into_any_element()
}

fn clip_preview(l: &Launcher, entry: &clip::Entry) -> Div {
    let image = entry.kind == "image";
    let content: AnyElement = if image {
        if l.clip.thumbs.contains(&entry.id) {
            img(clip::thumb(&clip::thumb_dir(), &entry.id))
                .size_full()
                .object_fit(gpui::ObjectFit::Contain)
                .into_any_element()
        } else {
            div().into_any_element()
        }
    } else {
        let text = l.clip.texts.get(&entry.id).unwrap_or(&entry.text);
        div()
            .w_full()
            .font_family(theme::MONO)
            .text_size(px(12.))
            .line_height(px(12. * 1.45))
            .text_color(theme::TEXT)
            .child(text.replace('\t', "    "))
            .into_any_element()
    };
    vstack(12.)
        .flex_1()
        .min_w_0()
        .h_full()
        .p(px(16.))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .rounded(px(14.))
                .bg(gpui::Rgba {
                    r: 0.,
                    g: 0.,
                    b: 0.,
                    a: 0.22,
                })
                .overflow_hidden()
                .p(px(14.))
                .child(content),
        )
        .child(small(
            l.clip_meta(entry).to_string(),
            theme::alpha(theme::TEXT, 0.62),
        ))
}

/// "LOGIN" → its username; other categories by name ("secure note").
fn item_subtitle(item: &Item) -> String {
    if item.category == "LOGIN" {
        item.user.clone()
    } else {
        item.category.to_lowercase().replacen('_', " ", 1)
    }
}

fn pass_row(model: &Entity<Launcher>, l: &Launcher, i: usize) -> AnyElement {
    let selected = l.current_index() == Some(i);
    match l.row(i) {
        Some(Row::Field(field)) => {
            let title = if field.is_otp() {
                "One-time code".to_string()
            } else {
                field.label.clone()
            };
            let (value, secret) = match &field.value {
                None => ("••••••••".to_string(), true),
                Some(v) => (v.split_whitespace().collect::<Vec<_>>().join(" "), false),
            };
            let row = row(model, format!("field-{}", field.id), i, selected, TALL_ROW).child(
                vstack(2.)
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().font_weight(theme::WEIGHT).child(title))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(theme::alpha(theme::TEXT, 0.6))
                            .when(secret, |d| d.font_family(theme::MONO))
                            .child(value),
                    ),
            );
            on_activate(row, model, i).into_any_element()
        }
        Some(Row::Pass(item)) => {
            let open = model.clone();
            let opened = item.clone();
            let row = row(model, format!("item-{}", item.id), i, selected, TALL_ROW)
                .child(letter_tile(&item.title, 30., 8.))
                .child(
                    vstack(2.)
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .font_weight(theme::WEIGHT)
                                .child(item.title.clone()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(theme::alpha(theme::TEXT, 0.6))
                                .child(item_subtitle(item)),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("fields-{}", item.id)))
                        .flex_none()
                        .size(px(22.))
                        .rounded(px(11.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .invisible()
                        .group_hover("row", |s| s.visible())
                        .hover(|s| s.bg(theme::FILL_HOVER))
                        .child(icon("chevron-right", 12., theme::TEXT))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            let item = opened.clone();
                            open.update(cx, |l, cx| l.show_fields(item, cx));
                        }),
                );
            // Click takes the password, like enter; the modifiers mean the same.
            on_activate(row, model, i).into_any_element()
        }
        _ => div().into_any_element(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FieldLook {
    Plain,
    Strong,
    Link,
}

/// One field of the detail pane; clicking takes it (shift: copy only).
fn pass_field(
    id: String,
    title: &str,
    value: String,
    secret: bool,
    look: FieldLook,
    keys: &[&str],
    take: impl Fn(bool, &mut App) + 'static,
) -> impl IntoElement {
    let bg = match look {
        FieldLook::Strong => theme::alpha(theme::WHITE, 0.1),
        FieldLook::Link => theme::TRANSPARENT,
        FieldLook::Plain => theme::alpha(theme::WHITE, 0.06),
    };
    hstack(12.)
        .id(SharedString::from(id))
        .w_full()
        .h(px(54.))
        .px(px(12.))
        .rounded(px(12.))
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(theme::FILL_HOVER))
        .on_click(move |event, _, cx| take(!event.modifiers().shift, cx))
        .child(
            vstack(2.)
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme::TEXT_DIM)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .truncate()
                        .text_color(if look == FieldLook::Link {
                            theme::ACCENT
                        } else {
                            theme::TEXT
                        })
                        .when(secret, |d| d.font_family(theme::MONO))
                        .child(value),
                ),
        )
        .when(!keys.is_empty(), |r| r.child(key_hint(keys, "")))
}

/// The selected item (or the item whose fields are listed): its title and what can be taken.
fn pass_detail(model: &Entity<Launcher>, l: &Launcher, item: &Item) -> Div {
    let take = |what: &'static str| {
        let model = model.clone();
        let item = item.clone();
        move |paste: bool, cx: &mut App| {
            let item = item.clone();
            model.update(cx, |l, cx| l.take_pass(item, what, paste, cx));
        }
    };
    let subtitle = if item.vault_name.is_empty() {
        item.category.to_lowercase().replacen('_', " ", 1)
    } else {
        item.vault_name.clone()
    };
    let url = item.urls.split(' ').next().unwrap_or_default().to_string();
    let mut detail = vstack(12.).flex_1().min_w_0().h_full().p(px(16.)).child(
        hstack(12.)
            .w_full()
            .pb(px(6.))
            .child(letter_tile(&item.title, 44., 12.))
            .child(
                vstack(2.)
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(16.))
                            .font_weight(theme::SEMIBOLD)
                            .child(item.title.clone()),
                    )
                    .child(small(subtitle, theme::alpha(theme::TEXT, 0.62))),
            ),
    );
    if !item.user.is_empty() {
        detail = detail.child(pass_field(
            format!("user-{}", item.id),
            "Username",
            item.user.clone(),
            false,
            FieldLook::Plain,
            &["ctrl", "↵"],
            take("username"),
        ));
    }
    detail = detail.child(pass_field(
        format!("password-{}", item.id),
        "Password",
        "••••••••••••".into(),
        true,
        FieldLook::Strong,
        &["↵"],
        take("password"),
    ));
    if l.pw.has_otp(item) == Some(true) {
        detail = detail.child(pass_field(
            format!("otp-{}", item.id),
            "One-time code",
            "••• •••".into(),
            true,
            FieldLook::Plain,
            &["alt", "↵"],
            take("otp"),
        ));
    }
    if !url.is_empty() {
        let shown = url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        let model = model.clone();
        detail = detail.child(pass_field(
            format!("url-{}", item.id),
            "Website",
            shown,
            false,
            FieldLook::Link,
            &[],
            move |_, cx| {
                let url = url.clone();
                model.update(cx, |l, cx| l.open_url(url, cx));
            },
        ));
    }
    detail
}

// ---- The footer

fn footer(model: &Entity<Launcher>, l: &Launcher) -> Div {
    let wipe = model.clone();
    hstack(14.)
        .w_full()
        .h(px(40.))
        .px(px(16.))
        .child(
            div()
                .max_w(px(280.))
                .truncate()
                .text_size(px(theme::SMALL))
                .text_color(if l.status_is_error() {
                    theme::WARN_TEXT
                } else {
                    theme::TEXT_DIM
                })
                .child(l.status()),
        )
        .when(l.mode == Mode::Clip && !l.clip.entries.is_empty(), |row| {
            row.child(
                pill("wipe", "Wipe")
                    .height(22.)
                    .on_click(move |_, cx| wipe.update(cx, |l, cx| l.clip_wipe(cx))),
            )
        })
        .child(div().flex_1())
        .children(
            l.hints()
                .into_iter()
                .map(|(keys, text)| key_hint(keys, text)),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn eliding() {
        assert_eq!(super::elide_middle("short", 10), "short");
        assert_eq!(super::elide_middle("abcdefghij", 5), "ab…ij");
    }

    #[test]
    fn subtitles() {
        let item = |category: &str| crate::services::launcher::passwords::Item {
            id: "i".into(),
            title: "T".into(),
            category: category.into(),
            vault: "v".into(),
            vault_name: String::new(),
            user: "me".into(),
            urls: String::new(),
        };
        assert_eq!(super::item_subtitle(&item("LOGIN")), "me");
        assert_eq!(super::item_subtitle(&item("SECURE_NOTE")), "secure note");
    }

    #[test]
    fn scope_names() {
        assert_eq!(super::super::Scope::Downloads.name(), "downloads");
    }
}
