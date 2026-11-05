//! The notification card (NotifCard.qml) and where it's shown: the popups, the history panel,
//! and the bar's bell.

use std::cell::RefCell;
use std::rc::Rc;

use chrono::{DateTime, Local};
use gpui::{
    AnyElement, App, Bounds, Context, Div, Entity, FocusHandle, KeyDownEvent, Pixels, Rgba,
    ScrollHandle, SharedString, Stateful, Window, canvas, div, img, prelude::*, px,
};

use super::{Entry, Notifications, SPACING};
use crate::bar::Bar;
use crate::panel::PanelKind;
use crate::services::icons::Image;
use crate::services::notifications::Urgency;
use crate::ui::{Width, bar_button, glass, hstack, icon, key_hint, label, small, vstack};
use crate::{theme, wayland};

pub const CARD_RADIUS: f32 = 18.;

/// The popups window: the newest few, top right. Hovering pauses their expiry.
pub struct Popups {
    model: Entity<Notifications>,
    /// The card bounds last given to the compositor (input and blur).
    regions: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    _observer: gpui::Subscription,
}

impl Popups {
    pub fn new(model: Entity<Notifications>, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            regions: Rc::default(),
            _observer: observer,
        }
    }
}

impl Render for Popups {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.clone();
        let n = model.read(cx);
        // Each card's bounds, as laid out this frame.
        let cards: Rc<RefCell<Vec<Bounds<Pixels>>>> = Rc::default();
        let mut column = vstack(SPACING).w_full();
        for entry in n.shown() {
            let id = entry.notification.id;
            let critical = entry.notification.urgency == Urgency::Critical;
            let mut body = vstack(10.).child(card(entry, "now".into(), 6));
            if !entry.notification.actions.is_empty() {
                // Two a row; one alone gets the whole row, as its text is often a sentence.
                body = body.child(crate::ui::columns(
                    entry.notification.actions.len().min(2),
                    8.,
                    entry.notification.actions.iter().map(|(key, text)| {
                        let (model, key) = (model.clone(), key.clone());
                        action_pill(format!("{id}-{key}"), text.clone())
                            .on_click(move |_, cx| {
                                model.update(cx, |n, cx| n.invoke(id, key.clone(), cx))
                            })
                            .into_any_element()
                    }),
                ));
            }
            let record = {
                let cards = cards.clone();
                canvas(move |b, _, _| cards.borrow_mut().push(b), |_, _, _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
            };
            let (activate, hide) = (model.clone(), model.clone());
            column = column.child(
                glass(CARD_RADIUS)
                    .id(("popup", id as usize))
                    .group("card")
                    .relative()
                    .w_full()
                    .p(px(12.))
                    .when(critical, |c| c.border_color(theme::alpha(theme::WARN, 0.6)))
                    .cursor_pointer()
                    .on_click(move |_, _, cx| activate.update(cx, |n, cx| n.activate(id, cx)))
                    .child(body)
                    .child(close_button(("hide", id as usize), false, move |_, cx| {
                        hide.update(cx, |n, cx| n.hide(id, cx))
                    }))
                    .child(record),
            );
        }

        // After the cards have been laid out: input over them only, blur behind each one.
        let regions = self.regions.clone();
        let apply = canvas(
            move |_, window, _| {
                let cards = std::mem::take(&mut *cards.borrow_mut());
                if *regions.borrow() != cards {
                    window.set_input_region(Some(&cards));
                    let blur: Vec<_> = cards.iter().map(|b| (*b, CARD_RADIUS)).collect();
                    wayland::set_blur(window, &blur);
                    *regions.borrow_mut() = cards;
                }
            },
            |_, _, _, _| {},
        )
        .size_0();

        let pause = model.clone();
        crate::ui::root()
            .child(
                div()
                    .id("popups")
                    .w_full()
                    .on_hover(move |hovered, _, cx| pause.update(cx, |n, _| n.set_paused(*hovered)))
                    .child(column),
            )
            .child(apply)
    }
}

/// The history panel. It takes the keyboard: up and down pick a notification (so does the
/// pointer), enter opens it, delete removes it and escape closes the panel.
pub struct Center {
    model: Entity<Notifications>,
    focus: FocusHandle,
    /// Of the notifications shown (`entries`).
    selected: usize,
    scroll: ScrollHandle,
    _observer: gpui::Subscription,
}

impl Center {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let model = crate::shell::Shell::global(cx)
            .read(cx)
            .notifications
            .clone();
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            model,
            focus,
            selected: 0,
            scroll: ScrollHandle::new(),
            _observer: observer,
        }
    }

    /// The selected notification's id.
    fn current(&self, cx: &App) -> Option<u32> {
        let n = self.model.read(cx);
        let entries = entries(n);
        entries
            .get(self.selected.min(entries.len().saturating_sub(1)))
            .map(|e| e.notification.id)
    }

    fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        if self.selected != i {
            self.selected = i;
            cx.notify();
        }
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let count = entries(self.model.read(cx)).len();
        let last = count.saturating_sub(1);
        let selected = self.selected.min(last);
        match event.keystroke.key.as_str() {
            "up" | "down" => {
                self.selected = if event.keystroke.key == "up" {
                    selected.saturating_sub(1)
                } else {
                    (selected + 1).min(last)
                };
                self.scroll.scroll_to_item(self.selected);
                cx.notify();
            }
            "enter" => {
                if let Some(id) = self.current(cx) {
                    self.open(id, window, cx);
                }
            }
            "delete" => {
                if let Some(id) = self.current(cx) {
                    self.model.update(cx, |n, cx| n.remove(id, cx));
                }
            }
            "escape" => window.remove_window(),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// The notification's default action, if it still has one: the panel closes first, so the
    /// keyboard is back on the desktop by the time the app asks for it.
    fn open(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.model.read(cx);
        if n.entry(id).is_some_and(openable) {
            window.remove_window();
            self.model
                .update(cx, |n, cx| n.invoke(id, "default".into(), cx));
        }
    }
}

/// What the history shows: transient notifications only pop up.
fn entries(n: &Notifications) -> Vec<&Entry> {
    n.history
        .iter()
        .filter(|e| !e.notification.transient)
        .collect()
}

/// Clicking it, or enter, runs its default action.
fn openable(entry: &Entry) -> bool {
    entry.live && entry.notification.has_default
}

impl Render for Center {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.clone();
        let n = model.read(cx);
        let (dnd_model, clear_model) = (model.clone(), model.clone());
        let dnd = bar_button(
            "dnd",
            hstack(6.)
                .child(icon(
                    if n.dnd { "moon" } else { "moon-outline" },
                    14.,
                    if n.dnd {
                        theme::SCREEN
                    } else {
                        theme::TEXT_DIM
                    },
                ))
                .child(small("DND", theme::TEXT_DIM)),
        )
        .rest(if n.dnd {
            theme::alpha(theme::SCREEN, 0.18)
        } else {
            theme::FILL
        })
        .radius(13.)
        .on_click(move |_, cx| dnd_model.update(cx, |n, cx| n.toggle_dnd(cx)));
        let clear = bar_button("clear", small("Clear", theme::TEXT_DIM))
            .rest(theme::FILL)
            .radius(13.)
            .on_click(move |_, cx| clear_model.update(cx, |n, cx| n.clear(cx)));

        let entries = entries(n);
        // Removing the last one moves the selection up.
        self.selected = self.selected.min(entries.len().saturating_sub(1));
        let list: AnyElement = if entries.is_empty() {
            div()
                .flex()
                .justify_center()
                .py(px(28.))
                .child(small(
                    if n.dnd {
                        "No notifications · Do not disturb is on"
                    } else {
                        "No notifications"
                    },
                    theme::TEXT_DIM,
                ))
                .into_any_element()
        } else {
            vstack(8.)
                .id("history")
                .max_h(px(560.))
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .children(entries.into_iter().enumerate().map(|(i, e)| {
                    history_row(e, i == self.selected, &model)
                        .on_hover(cx.listener(move |center, hovered, _, cx| {
                            if *hovered {
                                center.select(i, cx);
                            }
                        }))
                        .on_click(cx.listener({
                            let id = e.notification.id;
                            move |center, _, window, cx| center.open(id, window, cx)
                        }))
                }))
                .into_any_element()
        };

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|center, event, window, cx| center.key(event, window, cx)))
            .child(
                crate::panel::frame("Notifications")
                    .action(dnd)
                    .action(clear)
                    .footer(div().flex_1())
                    .footer(
                        hstack(16.)
                            .child(key_hint(&["↵"], "Open"))
                            .child(key_hint(&["del"], "Remove"))
                            .child(key_hint(&["esc"], "Close")),
                    )
                    .footer(div().flex_1())
                    .body(list),
            )
    }
}

/// A notification in the history; the selected one (hovered or picked with the arrows) is a
/// step lighter.
fn history_row(entry: &Entry, selected: bool, model: &Entity<Notifications>) -> Stateful<Div> {
    let id = entry.notification.id;
    let critical = entry.notification.urgency == Urgency::Critical;
    let remove = model.clone();
    div()
        .id(("entry", id as usize))
        .group("card")
        .relative()
        .w_full()
        .p(px(12.))
        .rounded(px(16.))
        .bg(if selected {
            theme::FILL_HOVER
        } else {
            theme::FILL
        })
        .when(critical, |d| {
            d.border_1().border_color(theme::alpha(theme::WARN, 0.5))
        })
        .when(openable(entry), |d| d.cursor_pointer())
        .child(card(entry, when(entry.time), 3))
        .child(close_button(
            ("remove", id as usize),
            selected,
            move |_, cx| remove.update(cx, |n, cx| n.remove(id, cx)),
        ))
}

/// The × in a card's corner, shown while the card is hovered (or always, when `shown`).
fn close_button(
    id: impl Into<gpui::ElementId>,
    shown: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .absolute()
        .top(px(8.))
        .right(px(8.))
        .when(!shown, |d| {
            d.invisible().group_hover("card", |s| s.visible())
        })
        .child(
            bar_button(id, icon("close", 12., theme::TEXT))
                .height(22.)
                .radius(11.)
                .padding(5.)
                .rest(theme::FILL_HOVER)
                .on_click(on_click),
        )
}

/// "14:02", or with the day when it's older than today.
fn when(time: DateTime<Local>) -> SharedString {
    if time.date_naive() == Local::now().date_naive() {
        time.format("%H:%M").to_string().into()
    } else {
        time.format("%a %H:%M").to_string().into()
    }
}

/// What a notification says: its icon (or a coloured letter), app name and time, the summary,
/// the body (up to `body_lines` lines), and its picture under them unless it's an avatar.
fn card(entry: &Entry, time: SharedString, body_lines: usize) -> Div {
    let n = &entry.notification;
    let avatar = n.picture.as_ref().filter(|p| p.avatar).map(|p| &p.image);
    let thumb = n.picture.as_ref().filter(|p| !p.avatar).map(|p| &p.image);
    let tile: AnyElement = match avatar.or(n.icon.as_ref()) {
        Some(image) => picture(image)
            .size(px(32.))
            .rounded(px(9.))
            .into_any_element(),
        None => div()
            .size(px(32.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(9.))
            .bg(tint(&n.app))
            .text_color(theme::WHITE)
            .font_weight(theme::SEMIBOLD)
            .text_size(px(14.))
            .child(
                n.app
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string(),
            )
            .into_any_element(),
    };

    let dim = theme::alpha(theme::TEXT, 0.66);
    let app_color = if n.urgency == Urgency::Critical {
        theme::WARN_TEXT
    } else {
        dim
    };
    let mut text = vstack(3.).flex_1().min_w_0().child(
        hstack(0.)
            .pr(px(26.))
            .child(small(n.app.clone(), app_color).flex_1().truncate())
            .child(
                div()
                    .font_family(theme::MONO)
                    .text_size(px(11.))
                    .text_color(dim)
                    .child(time),
            ),
    );
    if !n.summary.is_empty() {
        let summary = n.summary.split_whitespace().collect::<Vec<_>>().join(" ");
        text = text.child(label(summary, theme::TEXT).font_weight(theme::SEMIBOLD));
    }
    if !n.body.is_empty() {
        text = text
            .child(small(n.body.clone(), theme::alpha(theme::TEXT, 0.82)).line_clamp(body_lines));
    }
    if let Some(image) = thumb {
        // The box sets the size: the image is laid out over it, not in the flow, since gpui
        // gives an image its own aspect ratio and the box would otherwise take that.
        text = text.child(
            div()
                .relative()
                .mt(px(6.))
                .w_full()
                .h(px(96.))
                .flex_none()
                .overflow_hidden()
                .rounded(px(9.))
                .child(
                    picture(image)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(9.)),
                ),
        );
    }
    hstack(12.)
        .items_start()
        .w_full()
        .child(div().flex_none().child(tile))
        .child(text)
}

fn picture(image: &Image) -> gpui::Img {
    match image {
        Image::Svg(path) => img(path.clone()),
        Image::Raster(image) => img(image.clone()).object_fit(gpui::ObjectFit::Cover),
    }
}

/// A stable colour per app, for the letter tile.
fn tint(app: &str) -> Rgba {
    const COLORS: [u32; 8] = [
        0x8a3f78, 0x2f8f7f, 0x5865c9, 0x2b4a6e, 0x9a5b2e, 0x3f7a4a, 0x7a4fa8, 0xa8434f,
    ];
    let h = app
        .chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
    gpui::rgb(COLORS[h as usize % COLORS.len()])
}

fn action_pill(id: String, text: String) -> crate::ui::BarButton {
    bar_button(SharedString::from(id), small(text, theme::TEXT))
        .height(30.)
        .width(Width::Full)
        .radius(15.)
        .centered()
        .rest(theme::FILL)
}

/// The bar's bell: a moon while do-not-disturb is on, with a dot while it has held
/// notifications back that haven't been seen. Click opens the history, right click toggles
/// do-not-disturb.
pub fn indicator(
    model: &Entity<Notifications>,
    open: bool,
    bar: Entity<Bar>,
    cx: &App,
) -> impl IntoElement + use<> {
    let n = model.read(cx);
    let dnd = model.clone();
    let button = bar_button(
        "bell",
        icon(
            if n.dnd { "moon" } else { "bell" },
            17.,
            if n.dnd { theme::SCREEN } else { theme::TEXT },
        ),
    )
    .active(open)
    .on_press(move |_, window, cx| {
        bar.update(cx, |bar, cx| {
            bar.toggle_panel(PanelKind::Notifications, window, cx)
        });
    })
    .on_right(move |_, _, cx| dnd.update(cx, |n, cx| n.toggle_dnd(cx)));
    // A dot with a ring in the bar's colour.
    let dot = (n.unread() > 0).then(|| {
        div()
            .absolute()
            .top(px(2.5))
            .right(px(4.5))
            .size(px(10.))
            .rounded(px(5.))
            .bg(theme::ACCENT)
            .border(px(1.5))
            .border_color(gpui::rgba(0x081216cc))
    });
    div().relative().child(button).children(dot)
}
