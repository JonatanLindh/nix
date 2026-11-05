//! The system tray (StatusNotifierItem + DBusMenu), via the `system-tray` crate.
//!
//! Icons are resolved here, once per change, into gpui images: theme icons by name (see
//! `icons`), or the pixel data an item sends. Raster icons come mostly grey, so coloured app icons sit with the monochrome bar icons; an item asking
//! for attention keeps its colour.

use std::collections::HashMap;
use std::time::Duration;

use futures::SinkExt;
use futures::channel::mpsc::Sender;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::RenderImage;
use system_tray::client::{ActivateRequest, Client};
use system_tray::item::{IconPixmap, StatusNotifierItem};
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};
use tokio::sync::OnceCell;

use super::{Result, icons};

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The item's bus address, which every request names.
    pub address: String,
    pub status: Status,
    pub icon: Option<Icon>,
    /// Activating it just shows its menu.
    pub only_menu: bool,
    pub menu: Option<Menu>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Passive,
    Active,
    NeedsAttention,
}

#[derive(Clone)]
pub enum Icon {
    /// Drawn tinted, like the bar's own icons (or as-is when asking for attention).
    Svg(PathBuf),
    Raster {
        grey: Arc<RenderImage>,
        color: Arc<RenderImage>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Menu {
    /// The menu's object path, which activating an entry names.
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    /// `Some(checked)` for checkmark and radio entries.
    pub checked: Option<bool>,
    pub children: Vec<Entry>,
}

async fn client() -> Result<&'static Client> {
    static CLIENT: OnceCell<Client> = OnceCell::const_new();
    Ok(CLIENT.get_or_try_init(Client::new).await?)
}

pub fn watch() -> futures::channel::mpsc::Receiver<Vec<Item>> {
    crate::runtime::stream(4, async |out| {
        super::supervise("tray", Duration::from_secs(10), out, run).await
    })
}

async fn run(mut out: Sender<Vec<Item>>) -> Result {
    let client = client().await?;
    let mut events = client.subscribe();
    let mut icons = IconCache::default();
    loop {
        out.send(snapshot(client, &mut icons)).await?;
        events.recv().await?;
        // Items announce themselves property by property; take the burst as one change.
        while let Ok(Ok(_)) = tokio::time::timeout(Duration::from_millis(50), events.recv()).await {
        }
    }
}

fn snapshot(client: &Client, icons: &mut IconCache) -> Vec<Item> {
    let items = client.items();
    let items = items.lock().unwrap();
    let mut list: Vec<Item> = items
        .iter()
        .map(|(address, (item, menu))| Item {
            address: address.clone(),
            status: match item.status {
                system_tray::item::Status::Passive => Status::Passive,
                system_tray::item::Status::NeedsAttention => Status::NeedsAttention,
                _ => Status::Active,
            },
            icon: icons.get(item),
            only_menu: item.item_is_menu,
            menu: item
                .menu
                .clone()
                .zip(menu.as_ref())
                .map(|(path, menu)| Menu {
                    path,
                    entries: entries(menu),
                }),
        })
        .collect();
    // A stable order, so icons don't shuffle as items update.
    list.sort_by(|a, b| a.address.cmp(&b.address));
    list
}

fn entries(menu: &TrayMenu) -> Vec<Entry> {
    fn convert(items: &[MenuItem]) -> Vec<Entry> {
        items
            .iter()
            .filter(|i| i.visible)
            .map(|i| Entry {
                id: i.id,
                // DBusMenu marks access keys with an underscore ("_Quit"); "__" is a literal one.
                label: i
                    .label
                    .as_deref()
                    .unwrap_or_default()
                    .replace("__", "\0")
                    .replace('_', "")
                    .replace('\0', "_"),
                enabled: i.enabled,
                separator: i.menu_type == MenuType::Separator,
                checked: (i.toggle_type != ToggleType::CannotBeToggled)
                    .then_some(i.toggle_state == ToggleState::On),
                children: convert(&i.submenu),
            })
            .collect()
    }
    convert(&menu.submenus)
}

/// The default action (a left click).
pub async fn activate(address: String) {
    request(ActivateRequest::Default {
        address,
        x: 0,
        y: 0,
    })
    .await;
}

/// The secondary action (a middle click).
pub async fn secondary(address: String) {
    request(ActivateRequest::Secondary {
        address,
        x: 0,
        y: 0,
    })
    .await;
}

/// Tells the item its menu is about to show, so menus built lazily fill in.
pub async fn menu_opening(address: String, menu_path: String) {
    if let Ok(client) = client().await
        && let Err(e) = client.about_to_show_menuitem(address, menu_path, 0).await
    {
        eprintln!("tray: about to show: {e}");
    }
}

pub async fn activate_entry(address: String, menu_path: String, id: i32) {
    request(ActivateRequest::MenuItem {
        address,
        menu_path,
        submenu_id: id,
    })
    .await;
}

async fn request(request: ActivateRequest) {
    match client().await {
        Ok(client) => {
            if let Err(e) = client.activate(request).await {
                eprintln!("tray: {e}");
            }
        }
        Err(e) => eprintln!("tray: {e}"),
    }
}

/// Icons by what they were made from, so an unchanged icon keeps its handle (and stays
/// uploaded) across snapshots.
#[derive(Default)]
struct IconCache(HashMap<String, Option<Icon>>);

impl IconCache {
    fn get(&mut self, item: &StatusNotifierItem) -> Option<Icon> {
        let attention = matches!(item.status, system_tray::item::Status::NeedsAttention);
        let (name, pixmaps) = if attention && item.attention_icon_name.is_some() {
            (&item.attention_icon_name, &item.attention_icon_pixmap)
        } else {
            (&item.icon_name, &item.icon_pixmap)
        };
        let name = name.as_deref().filter(|n| !n.is_empty());
        let key = match (name, pixmaps) {
            (Some(n), _) => format!("name:{n}:{:?}", item.icon_theme_path),
            (None, Some(p)) => format!("pixmap:{}", hash(p)),
            (None, None) => return None,
        };
        self.0
            .entry(key)
            .or_insert_with(|| {
                name.and_then(|n| by_name(n, item.icon_theme_path.as_deref()))
                    .or_else(|| pixmaps.as_deref().and_then(from_pixmaps))
            })
            .clone()
    }
}

fn hash(pixmaps: &[IconPixmap]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in pixmaps {
        (p.width, p.height, &p.pixels).hash(&mut h);
    }
    h.finish()
}

fn by_name(name: &str, theme_path: Option<&str>) -> Option<Icon> {
    let path = icons::path_for(name, theme_path)?;
    if path.extension().is_some_and(|e| e == "svg") {
        return Some(Icon::Svg(path));
    }
    let rgba = ::image::open(&path).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    raster(w, h, rgba.into_raw())
}

/// The pixmap closest to (but not under) the size it's shown at, else the largest.
fn from_pixmaps(pixmaps: &[IconPixmap]) -> Option<Icon> {
    const WANT: i32 = 30;
    let pick = pixmaps
        .iter()
        .filter(|p| p.width >= WANT)
        .min_by_key(|p| p.width)
        .or_else(|| pixmaps.iter().max_by_key(|p| p.width))?;
    // SNI pixmaps are ARGB32 in network byte order.
    let rgba: Vec<u8> = pick
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[1], p[2], p[3], p[0]])
        .collect();
    raster(pick.width as u32, pick.height as u32, rgba)
}

/// Raster icons are scaled to this once, with a good filter: the GPU's plain linear filter
/// leaves a large icon shrunk to 15px grainy.
const RASTER_SIZE: u32 = 32;

fn raster(width: u32, height: u32, rgba: Vec<u8>) -> Option<Icon> {
    let (width, height, rgba) = shrink(width, height, rgba);
    // 85% towards grey, like Quickshell's `saturation: -0.85`.
    let grey = icons::desaturate(rgba.clone(), 0.85);
    Some(Icon::Raster {
        grey: icons::raster(width, height, grey)?,
        color: icons::raster(width, height, rgba)?,
    })
}

fn shrink(width: u32, height: u32, rgba: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let largest = width.max(height);
    if largest <= RASTER_SIZE {
        return (width, height, rgba);
    }
    let Some(img) = ::image::RgbaImage::from_raw(width, height, rgba.clone()) else {
        return (width, height, rgba);
    };
    let (w, h) = (
        (width * RASTER_SIZE / largest).max(1),
        (height * RASTER_SIZE / largest).max(1),
    );
    let small = ::image::imageops::resize(&img, w, h, ::image::imageops::FilterType::Lanczos3);
    (w, h, small.into_raw())
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Icon::Svg(path) => write!(f, "Svg({})", path.display()),
            Icon::Raster { .. } => f.write_str("Raster"),
        }
    }
}

/// The same icon: the same file, or the same decoded pixels (the cache hands out one copy).
impl PartialEq for Icon {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Icon::Svg(a), Icon::Svg(b)) => a == b,
            (Icon::Raster { color: a, .. }, Icon::Raster { color: b, .. }) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}
