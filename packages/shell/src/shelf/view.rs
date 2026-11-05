//! The shelf's bar button and its window.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, Div, Entity, ExternalDragPayload, ExternalPaths, FileDragPaths, Pixels,
    Rgba, SharedString, canvas, div, point, prelude::*, px, size,
};

use super::Shelf;
use crate::services::shelf::{Action, Item, Job, LinkQr, Options, Peer, Qr};
use crate::ui::controls::pill;
use crate::ui::{Width, bar_button, divider, glass, hstack, icon, label, small, vstack};
use crate::{theme, wayland};

const CARD_RADIUS: f32 = 18.;

/// The bar button (ShelfButton.qml): shows or hides the shelf on this bar's monitor, with a
/// count. Also a drop target: files dropped on it go on the shelf, and a drag that rests on it
/// opens the shelf so the drop can land there instead.
pub fn button(
    model: &Entity<Shelf>,
    monitor: Option<String>,
    cx: &App,
) -> impl IntoElement + use<> {
    let shelf = model.read(cx);
    let shown_here = monitor.as_deref().is_some_and(|m| shelf.shown_on(m));
    let count = shelf.items.len();
    let (toggle, drop, spring) = (model.clone(), model.clone(), model.clone());
    let (toggle_on, drop_on, spring_on) = (monitor.clone(), monitor.clone(), monitor);
    div()
        .id("shelf-button")
        .rounded(px(theme::BUTTON_RADIUS))
        .drag_over::<ExternalPaths>(|style, _, _, _| style.bg(theme::alpha(theme::ACCENT, 0.32)))
        .on_drag_move::<ExternalPaths>(move |event, _, cx| {
            let over = event.bounds.contains(&event.event.position);
            let monitor = over.then(|| spring_on.clone()).flatten();
            spring.update(cx, |shelf, cx| shelf.drag_over_button(monitor, cx));
        })
        .on_drop::<ExternalPaths>(move |paths, _, cx| {
            let paths = paths
                .paths()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            drop.update(cx, |shelf, cx| shelf.dropped(drop_on.clone(), paths, cx));
        })
        .child(
            bar_button("shelf", icon("inbox", 17., theme::TEXT))
                .active(shown_here)
                .badge((count > 0).then(|| count.to_string()))
                .on_click(move |_, cx| {
                    if let Some(monitor) = &toggle_on {
                        toggle.update(cx, |shelf, cx| shelf.toggle(monitor, cx));
                    }
                }),
        )
}

/// The shelf window: the card at its top-left, the rest see-through and click-through.
pub struct Window {
    model: Entity<Shelf>,
    /// The card's bounds as last given to the compositor (input and blur).
    region: Rc<RefCell<Option<Bounds<Pixels>>>>,
    _observer: gpui::Subscription,
}

impl Window {
    pub fn new(model: Entity<Shelf>, _: &mut gpui::Window, cx: &mut Context<Self>) -> Self {
        let observer = cx.observe(&model, |_, _, cx| cx.notify());
        Self {
            model,
            region: Rc::default(),
            _observer: observer,
        }
    }
}

impl Render for Window {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let region = self.region.clone();
        // Only the card takes clicks and is blurred behind.
        let fit = canvas(
            move |bounds, window, _| {
                if *region.borrow() != Some(bounds) {
                    window.set_input_region(Some(&[bounds]));
                    wayland::set_blur(window, &[(bounds, CARD_RADIUS)]);
                    *region.borrow_mut() = Some(bounds);
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        crate::ui::root().child(
            div()
                .relative()
                .flex_none()
                .w_full()
                .child(card(&self.model, cx))
                .child(fit),
        )
    }
}

fn card(model: &Entity<Shelf>, cx: &App) -> impl IntoElement {
    let shelf = model.read(cx);
    let mut body = vstack(2.).child(header(model, shelf));
    if shelf.items.is_empty() {
        body = body.child(
            div()
                .flex()
                .justify_center()
                .py(px(18.))
                .px(px(8.))
                .child(small("Drop files here from any app", theme::TEXT_DIM)),
        );
    } else {
        let places = places(&shelf.items);
        for item in &shelf.items {
            let picked = shelf.selected.contains(&item.path);
            body = body.child(item_row(
                model,
                item,
                picked,
                places.get(&item.path).cloned(),
            ));
        }
        if shelf.items.len() > 1 {
            body = body.child(drag_all(&shelf.items));
        }
        body = body.child(tools(model, shelf));
    }
    // Outside the items: a share runs on after the shelf is cleared, and stops from here.
    let shares = &shelf.shares;
    if let Some(job) = shares.wormhole.as_ref().filter(|_| shares.wormhole_card) {
        body = body.child(wormhole_card(
            model,
            job,
            shares.wormhole_sending,
            shares.wormhole_copied,
        ));
    }
    if let Some(job) = shares.funnel.as_ref().filter(|_| shares.funnel_card) {
        body = body.child(funnel_card(model, job));
    }
    if let Some(status) = &shelf.status {
        body = body.child(status_line(
            &status.text,
            status.error,
            !shelf.busy.is_empty(),
        ));
    }

    let drop = model.clone();
    glass(CARD_RADIUS)
        .id("shelf-card")
        .group("shelf-card")
        .relative()
        .w_full()
        .p(px(8.))
        .on_drop::<ExternalPaths>(move |paths, _, cx| {
            let paths = paths
                .paths()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            drop.update(cx, |shelf, cx| shelf.dropped(None, paths, cx));
        })
        .child(body)
        // "Drop to add" over everything while files hover. Its own `drag_over`, not
        // `group_drag_over` on the card: gpui only applies drag-over styles to an element with a
        // hitbox, and the group variant doesn't give it one.
        .child(
            div()
                .absolute()
                .inset_0()
                .p(px(6.))
                .invisible()
                .drag_over::<ExternalPaths>(|s, _, _, _| s.visible())
                .child(
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(13.))
                        .bg(gpui::rgba(0x162428eb))
                        .border(px(1.5))
                        .border_color(theme::ACCENT)
                        .child("Drop to add to the shelf"),
                ),
        )
}

fn header(model: &Entity<Shelf>, shelf: &Shelf) -> impl IntoElement {
    let count = shelf.items.len();
    let title = hstack(5.)
        .child(label("Shelf", theme::TEXT).font_weight(theme::SEMIBOLD))
        .child(label(format!("· {count}"), theme::alpha(theme::TEXT, 0.6)));
    let mut header = hstack(8.)
        .px(px(6.))
        .py(px(4.))
        .child(icon("inbox", 16., theme::TEXT))
        .child(title.flex_1());
    if count > 0 {
        // Results are deleted with the rest; say so.
        let temps = shelf.items.iter().filter(|i| i.temp).count();
        let text = if temps > 0 {
            format!("Clear · deletes {temps}")
        } else {
            "Clear".into()
        };
        let clear = model.clone();
        header = header.child(
            bar_button("clear", small(text, theme::TEXT_DIM))
                .on_click(move |_, cx| clear.update(cx, |shelf, cx| shelf.clear(cx))),
        );
    }
    let hide = model.clone();
    header.child(
        bar_button("hide", icon("close", 12., theme::TEXT_DIM))
            .radius(13.)
            .padding(7.)
            .on_click(move |_, cx| hide.update(cx, |shelf, cx| shelf.hide(cx))),
    )
}

/// Each kind's icon sits on its own tint.
fn tint(kind: &str) -> Rgba {
    gpui::rgb(match kind {
        "folder" => 0x3f78b8,
        "image" => 0x8c5cb0,
        "code" => 0x2f8f7f,
        "archive" => 0x7a6a4a,
        "video" => 0xb0753f,
        "link" => 0x3f8f5a,
        "text" => 0x6a7a8a,
        _ => 0xb0533f,
    })
}

/// What a drag from the shelf carries: files, and a label for the chip under the pointer.
#[derive(Clone)]
struct Dragged {
    paths: Vec<PathBuf>,
    label: SharedString,
}

/// The chip that follows the pointer until the drag leaves the window.
struct DragChip(SharedString);

impl Render for DragChip {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(12.))
            .py(px(5.))
            .rounded(px(14.))
            .bg(gpui::rgba(0x7fd1c0d9))
            .text_color(theme::INK)
            .text_size(px(theme::SMALL))
            .font_family(theme::FONT)
            .child(self.0.clone())
    }
}

/// Makes `element` drag out as `dragged`'s files.
fn drag_source(element: gpui::Stateful<Div>, dragged: Dragged) -> gpui::Stateful<Div> {
    element
        .on_drag(dragged, |d: &Dragged, _, _, cx| {
            let label = d.label.clone();
            cx.new(|_| DragChip(label))
        })
        .external_drag_payload::<Dragged>(|d, _, _| {
            let entries = d.paths.iter().map(|p| (p.clone(), p.is_dir()));
            Some(ExternalDragPayload::Files(FileDragPaths::new(entries)))
        })
}

/// One item: drags out as its file, click selects, × removes.
fn item_row(
    model: &Entity<Shelf>,
    item: &Item,
    picked: bool,
    place: Option<String>,
) -> impl IntoElement {
    let tile = div()
        .flex_none()
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .bg(tint(&item.kind))
        .child(icon(&item.kind, 15., theme::WHITE));
    let content = hstack(10.)
        .w_full()
        .child(tile)
        .child(label(item.name.clone(), theme::TEXT).truncate())
        .children(place.map(|p| small(p, theme::TEXT_DIM)))
        .child(div().flex_1())
        .when(item.temp, |row| row.child(temp_badge()));

    let (select, remove) = (model.clone(), model.clone());
    let (path, remove_path) = (item.path.clone(), item.path.clone());
    let row = bar_button(SharedString::from(format!("item-{}", item.path)), content)
        .height(40.)
        .width(Width::Full)
        .radius(10.)
        .padding(8.)
        .active(picked)
        .outline(picked.then_some(theme::ACCENT))
        .on_click(move |_, cx| select.update(cx, |shelf, cx| shelf.select(path.clone(), cx)));

    let remove = remove_button(remove, remove_path, item.temp);

    let dragged = Dragged {
        paths: vec![PathBuf::from(&item.path)],
        label: item.name.clone().into(),
    };
    drag_source(
        div()
            .id(SharedString::from(format!("drag-{}", item.path)))
            .group("item")
            .relative(),
        dragged,
    )
    .child(row)
    .child(remove)
}

/// The × over a row's right end, shown while the row is hovered; a bin for the shelf's own
/// results, which go when removed.
fn remove_button(model: Entity<Shelf>, path: String, temp: bool) -> impl IntoElement {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(6.))
        .flex()
        .items_center()
        .invisible()
        .group_hover("item", |s| s.visible())
        .child(
            bar_button(
                SharedString::from(format!("remove-{path}")),
                icon(
                    if temp { "trash" } else { "close" },
                    if temp { 13. } else { 11. },
                    theme::TEXT,
                ),
            )
            .height(22.)
            .radius(11.)
            .padding(5.)
            .on_click(move |_, cx| model.update(cx, |shelf, _| shelf.remove(path.clone()))),
        )
}

fn temp_badge() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(18.))
        .px(px(7.))
        .rounded(px(9.))
        .bg(theme::FILL)
        .border_1()
        .border_color(theme::GLASS_BORDER)
        .text_size(px(10.))
        .font_weight(theme::SEMIBOLD)
        .text_color(theme::TEXT_DIM)
        .child("Temp")
}

fn drag_all(items: &[Item]) -> impl IntoElement {
    let dragged = Dragged {
        paths: items.iter().map(|i| PathBuf::from(&i.path)).collect(),
        label: format!("{} items", items.len()).into(),
    };
    let target = drag_source(div().id("drag-all"), dragged)
        .h(px(36.))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .rounded(px(10.))
        .border_1()
        .border_color(theme::alpha(theme::TEXT, 0.22))
        .cursor_grab()
        .child(icon("grip", 15., theme::TEXT_DIM))
        .child(label(format!("Drag all {}", items.len()), theme::TEXT_DIM));
    div().py(px(4.)).child(target)
}

/// Tools for the targets: up to six tiles, with the rest behind "More".
fn tools(model: &Entity<Shelf>, shelf: &Shelf) -> impl IntoElement {
    let targets = shelf.targets().len();
    let heading = if !shelf.selected.is_empty() {
        format!(
            "For {targets} selected file{}",
            if targets == 1 { "" } else { "s" }
        )
    } else if targets == 1 {
        "For this file".into()
    } else {
        format!("For all {targets} files")
    };

    let (inline, rest) = if shelf.actions.len() > 6 {
        let more = Action {
            id: "more",
            label: "More",
            icon: "more",
            options: None,
        };
        let mut inline = shelf.actions[..5].to_vec();
        inline.push(more);
        (inline, &shelf.actions[5..])
    } else {
        (shelf.actions.clone(), &[][..])
    };

    let tiles = crate::ui::columns(
        3,
        4.,
        inline
            .into_iter()
            .map(|a| tool_tile(model, shelf, a).into_any_element()),
    );
    let mut section = vstack(4.)
        .py(px(6.))
        .child(div().px(px(4.)).child(divider()))
        .child(
            div()
                .px(px(8.))
                .pt(px(6.))
                .pb(px(2.))
                .child(small(heading, theme::TEXT_DIM)),
        )
        .child(tiles);

    match shelf.open.as_deref() {
        Some("more") => {
            for action in rest {
                section = section.child(menu_row(model, action));
            }
        }
        Some("taildrop") => section = section.child(taildrop(model, shelf)),
        Some("qr") => {
            if let Some(qr) = &shelf.link_qr {
                section = section.child(link_qr_card(qr));
            }
        }
        Some(id) => {
            if let Some((id, options)) = shelf
                .actions
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| a.options.as_ref().map(|o| (a.id, o)))
            {
                section = section.child(choices(model, id, options));
            }
        }
        None => {}
    }
    section
}

fn tool_tile(model: &Entity<Shelf>, shelf: &Shelf, action: Action) -> impl IntoElement {
    let open = shelf.is_on(action.id);
    let working = shelf.busy.iter().any(|b| b == action.id);
    let ink = theme::alpha(theme::TEXT, if working { 0.42 } else { 0.85 });
    let content = vstack(4.)
        .items_center()
        .child(icon(action.icon, 17., ink))
        .child(div().text_size(px(11.)).text_color(ink).child(action.label));
    let model = model.clone();
    bar_button(SharedString::from(format!("tool-{}", action.id)), content)
        .height(52.)
        .width(Width::Full)
        .radius(12.)
        .padding(0.)
        .centered()
        .active(open)
        .on_click(move |_, cx| model.update(cx, |shelf, cx| shelf.activate(action.clone(), cx)))
}

/// A compact list row, for the tools behind "More".
fn menu_row(model: &Entity<Shelf>, action: &Action) -> impl IntoElement {
    let ink = theme::alpha(theme::TEXT, 0.85);
    let (model, action) = (model.clone(), action.clone());
    bar_button(
        SharedString::from(format!("more-{}", action.id)),
        hstack(10.)
            .child(icon(action.icon, 17., ink))
            .child(label(action.label, ink)),
    )
    .height(32.)
    .width(Width::Full)
    .radius(9.)
    .on_click(move |_, cx| model.update(cx, |shelf, cx| shelf.activate(action.clone(), cx)))
}

/// The Taildrop targets: your own devices, the offline ones greyed out.
fn taildrop(model: &Entity<Shelf>, shelf: &Shelf) -> impl IntoElement {
    let heading = if shelf.peers_loading {
        "Looking for devices…"
    } else if shelf.peers.is_empty() {
        "No devices to send to"
    } else {
        "Send with Taildrop to"
    };
    vstack(2.)
        .pt(px(4.))
        .child(
            div()
                .px(px(8.))
                .py(px(2.))
                .child(small(heading, theme::alpha(theme::TEXT, 0.6)).text_size(px(TINY))),
        )
        .children(shelf.peers.iter().map(|peer| peer_row(model, peer)))
}

fn peer_row(model: &Entity<Shelf>, peer: &Peer) -> impl IntoElement {
    let dot = if peer.online {
        theme::ONLINE
    } else {
        theme::alpha(theme::TEXT, 0.3)
    };
    let content = hstack(10.)
        .w_full()
        .child(
            div()
                .flex_none()
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(8.)).rounded(px(4.)).bg(dot)),
        )
        .child(
            label(
                peer.name.clone(),
                if peer.online {
                    theme::TEXT
                } else {
                    theme::alpha(theme::TEXT, 0.4)
                },
            )
            .flex_1()
            .truncate(),
        )
        .when(!peer.online, |row| {
            row.child(small("Offline", theme::alpha(theme::TEXT, 0.5)).text_size(px(TINY)))
        });
    let row = bar_button(SharedString::from(format!("peer-{}", peer.name)), content)
        .height(32.)
        .width(Width::Full)
        .radius(9.);
    // An offline device can't take files: no hover, no click.
    if !peer.online {
        return row;
    }
    let (model, name) = (model.clone(), peer.name.clone());
    row.on_click(move |_, cx| model.update(cx, |shelf, cx| shelf.taildrop(name.clone(), cx)))
}

/// QML's Theme.tiny, for the cards' fine print.
const TINY: f32 = 11.;

/// A card under the tools: a faint fill, rounded.
fn share_card() -> Div {
    div()
        .mt(px(6.))
        .w_full()
        .p(px(12.))
        .rounded(px(14.))
        .bg(theme::alpha(theme::WHITE, 0.07))
}

/// A link's (or note's) address as a QR, to open it on a phone.
fn link_qr_card(qr: &LinkQr) -> impl IntoElement {
    share_card().child(
        vstack(10.)
            .items_center()
            .child(qr_code(&qr.qr, 136., 8.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(fine_mono(qr.text.clone()).truncate()),
            ),
    )
}

/// Wormhole: the code and its QR until the other side has it.
fn wormhole_card(
    model: &Entity<Shelf>,
    job: &Job,
    sending: bool,
    copied: bool,
) -> impl IntoElement {
    let code = job.code.clone().unwrap_or_default();
    let words = vstack(4.)
        .flex_1()
        .min_w_0()
        .child(small("Wormhole code", theme::alpha(theme::TEXT, 0.6)).text_size(px(TINY)))
        .child(
            div()
                .font_family(theme::MONO)
                .text_size(px(15.))
                .line_height(px(17.))
                .text_color(theme::TEXT)
                .child(code.clone()),
        )
        .child(
            small(
                "Scan with a wormhole app, or type the code",
                theme::alpha(theme::TEXT, 0.55),
            )
            .text_size(px(TINY)),
        );
    let top = hstack(12.)
        .items_start()
        .child(qr_code(&job.qr, 96., 6.))
        .child(words);

    let copy_model = model.clone();
    let copy = bar_button(
        "wormhole-copy",
        hstack(8.)
            .w_full()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .font_family(theme::MONO)
                    .text_size(px(11.))
                    .text_color(theme::alpha(theme::TEXT, 0.8))
                    .truncate()
                    .child(format!("wormhole receive {code}")),
            )
            .child(
                small(
                    if copied { "Copied" } else { "Copy" },
                    theme::alpha(theme::TEXT, 0.6),
                )
                .text_size(px(TINY)),
            ),
    )
    .height(32.)
    .width(Width::Full)
    .radius(9.)
    .rest(theme::alpha(gpui::rgb(0x000000), 0.22))
    .on_click(move |_, cx| copy_model.update(cx, |shelf, cx| shelf.copy_wormhole(cx)));

    // A dot with a soft halo while it waits or sends.
    let dot = div()
        .flex_none()
        .size(px(13.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.5))
        .bg(theme::alpha(theme::ACCENT, 0.28))
        .child(div().size(px(7.)).rounded(px(3.5)).bg(theme::ACCENT));
    let (cancel_model, id) = (model.clone(), job.id.clone());
    let state =
        hstack(8.)
            .child(dot)
            .child(
                small(
                    if sending {
                        "Sending…"
                    } else {
                        "Waiting for the other side…"
                    },
                    theme::TEXT_DIM,
                )
                .flex_1(),
            )
            .child(pill("wormhole-cancel", "Cancel").on_click(move |_, cx| {
                cancel_model.update(cx, |shelf, _| shelf.cancel(id.clone()))
            }));

    share_card().child(vstack(8.).child(top).child(copy).child(state))
}

/// Funnel: the public link and its QR until stopped.
fn funnel_card(model: &Entity<Shelf>, job: &Job) -> impl IntoElement {
    let (copy_model, stop_model, id) = (model.clone(), model.clone(), job.id.clone());
    let buttons = hstack(6.)
        .child(
            pill("funnel-copy", "Copy link")
                .on_click(move |_, cx| copy_model.update(cx, |shelf, cx| shelf.copy_funnel(cx))),
        )
        .child(
            pill("funnel-stop", "Stop sharing")
                .on_click(move |_, cx| stop_model.update(cx, |shelf, _| shelf.cancel(id.clone()))),
        );
    share_card().child(
        vstack(10.)
            .items_center()
            .child(qr_code(&job.qr, 136., 8.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(fine_mono(job.url.clone().unwrap_or_default()).truncate()),
            )
            .child(buttons)
            .child(
                small(
                    "Public via Tailscale Funnel until you stop it",
                    theme::alpha(theme::TEXT, 0.55),
                )
                .text_size(px(TINY)),
            ),
    )
}

/// An address in the cards: small monospace, one line.
fn fine_mono(text: String) -> Div {
    div()
        .min_w_0()
        .font_family(theme::MONO)
        .text_size(px(TINY))
        .text_color(theme::alpha(theme::TEXT, 0.8))
        .child(text)
}

/// A QR code on a light card, `side` pixels square with `pad` of quiet zone at least. Modules are
/// a whole number of device pixels each, laid on the pixel grid, so the edges stay sharp; what
/// that leaves over goes around the code.
fn qr_code(qr: &Qr, side: f32, pad: f32) -> impl IntoElement {
    let modules = qr.size.max(1) as f32;
    let runs = qr.runs().to_vec();
    let code = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let scale = window.scale_factor();
            let (x, y, module) = qr_grid(
                f32::from(bounds.origin.x) * scale,
                f32::from(bounds.origin.y) * scale,
                f32::from(bounds.size.width.min(bounds.size.height)) * scale,
                modules,
            );
            for run in &runs {
                let origin = point(
                    px((x + run.x as f32 * module) / scale),
                    px((y + run.y as f32 * module) / scale),
                );
                let extent = size(px(run.len as f32 * module / scale), px(module / scale));
                window.paint_quad(gpui::fill(Bounds::new(origin, extent), theme::INK));
            }
        },
    )
    .size_full();
    div()
        .flex_none()
        .size(px(side))
        .p(px(pad))
        .rounded(px(if side > 100. { 12. } else { 10. }))
        .bg(theme::WHITE)
        .child(code)
}

/// Where a QR of `modules` a side goes in the square at (`x`, `y`) of `side`, all in device
/// pixels: its top-left corner on the pixel grid and a whole-pixel module, centred.
fn qr_grid(x: f32, y: f32, side: f32, modules: f32) -> (f32, f32, f32) {
    let module = (side / modules).floor().max(1.);
    let spare = (side - module * modules) / 2.;
    ((x + spare).round(), (y + spare).round(), module)
}

/// The options of convert / resize, as pills.
fn choices(model: &Entity<Shelf>, id: &'static str, options: &Options) -> impl IntoElement {
    let pills = options.items.iter().map(|choice| {
        let (model, option) = (model.clone(), choice.id);
        bar_button(
            SharedString::from(format!("choice-{}", choice.id)),
            small(choice.label, theme::TEXT),
        )
        .height(26.)
        .radius(13.)
        .padding(10.)
        .rest(theme::FILL)
        .on_click(move |_, cx| {
            model.update(cx, |shelf, cx| shelf.run(id.into(), option.into(), cx))
        })
    });
    vstack(4.)
        .px(px(4.))
        .child(
            div()
                .px(px(8.))
                .py(px(4.))
                .child(small(options.label, theme::TEXT_DIM)),
        )
        .child(hstack(6.).flex_wrap().children(pills))
}

fn status_line(text: &str, error: bool, working: bool) -> impl IntoElement {
    let dot = if error {
        theme::WARN
    } else {
        theme::alpha(theme::ACCENT, if working { 0.5 } else { 1. })
    };
    hstack(8.)
        .px(px(8.))
        .pt(px(6.))
        .pb(px(2.))
        .child(div().flex_none().size(px(6.)).rounded(px(3.)).bg(dot))
        .child(small(
            text.to_string(),
            if error {
                theme::WARN_TEXT
            } else {
                theme::alpha(theme::TEXT, 0.75)
            },
        ))
}

/// Where items that share a name live, by path: just enough of their folder to tell them apart
/// ("Downloads", "a/src", "~"). Results are told apart by their "Temp" badge already.
fn places(items: &[Item]) -> HashMap<String, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut groups: HashMap<&str, Vec<&str>> = HashMap::new();
    for item in items.iter().filter(|i| !i.temp) {
        groups.entry(&item.name).or_default().push(&item.path);
    }
    let mut out = HashMap::new();
    for paths in groups.into_values().filter(|p| p.len() > 1) {
        let dirs: Vec<Vec<&str>> = paths
            .iter()
            .map(|p| {
                let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
                parts[..parts.len().saturating_sub(1)].to_vec()
            })
            .collect();
        let tail = |d: &[&str], k: usize| d[d.len().saturating_sub(k)..].join("/");
        for (n, path) in paths.iter().enumerate() {
            let d = &dirs[n];
            // The fewest trailing folders no other item's path ends in the same way.
            let mut k = 1;
            while k < d.len()
                && dirs
                    .iter()
                    .enumerate()
                    .any(|(m, o)| m != n && tail(o, k) == tail(d, k))
            {
                k += 1;
            }
            let dir = format!("/{}", d.join("/"));
            let place = if dir == home {
                "~".to_string()
            } else if k >= d.len() {
                dir
            } else {
                tail(d, k)
            };
            out.insert(path.to_string(), place);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::qr_grid;

    #[test]
    fn qr_on_the_pixel_grid() {
        // 120 px for 25 modules: 4 px each, the 20 px spare split around.
        assert_eq!(qr_grid(8., 8., 120., 25.), (18., 18., 4.));
        // Fractional origins (scaled) land on whole pixels.
        let (x, y, module) = qr_grid(10.4, 7.6, 150., 29.);
        assert_eq!((x.fract(), y.fract(), module), (0., 0., 5.));
        // Never smaller than a pixel.
        assert_eq!(qr_grid(0., 0., 10., 25.).2, 1.);
    }
}
