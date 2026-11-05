//! The processes behind screenshots and recordings (Screenshot.qml): grim for the stills the
//! capture overlay shows and cuts screenshots from, Hyprland's monitor and window lists for
//! window mode, wl-copy for the clipboard and notify-send for the notification afterwards.
//! (wf-recorder is run from `crate::screenshot`, which needs its pid to stop it.)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use gpui::RenderImage;
use serde::Deserialize;

use super::{Result, hyprland};
use crate::screenshot::geometry::Rect;

/// A monitor as the overlay needs it: where it is in the global layout (logical pixels).
#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub focused: bool,
}

/// One monitor's full-resolution stills, with and without the pointer.
pub struct Still {
    pub plain: Arc<RenderImage>,
    pub pointer: Arc<RenderImage>,
    /// Where they are on disk, to cut screenshots from.
    pub plain_path: PathBuf,
    pub pointer_path: PathBuf,
    /// In physical pixels.
    pub width: u32,
}

/// Everything the overlay shows as it opens.
pub struct Snapshot {
    pub monitors: Vec<Monitor>,
    /// The windows on show, global logical rectangles, topmost first.
    pub windows: Vec<Rect>,
    /// By monitor name; a monitor grim couldn't take is missing.
    pub stills: HashMap<String, Still>,
}

/// Where this run's stills go: under the runtime directory, one folder per opening so a save
/// still reading the last ones can't race the next.
pub fn stills_dir(opening: u64) -> PathBuf {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(runtime)
        .join("shell-screenshot")
        .join(opening.to_string())
}

pub fn pictures_dir() -> PathBuf {
    home().join("Pictures/Screenshots")
}

pub fn videos_dir() -> PathBuf {
    home().join("Videos/Screencasts")
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("/"), PathBuf::from)
}

/// The monitors, the windows on show and a still of every monitor, taken before anything of
/// ours is on screen.
pub async fn snapshot(dir: PathBuf) -> Snapshot {
    let (monitors, clients) = match tokio::try_join!(
        hyprland::query::<Vec<HyprMonitor>>("j/monitors"),
        hyprland::query::<Vec<Client>>("j/clients"),
    ) {
        Ok(lists) => lists,
        Err(e) => {
            eprintln!("screenshot: asking Hyprland: {e}");
            (Vec::new(), Vec::new())
        }
    };
    let windows = shown_windows(&monitors, &clients);
    let takes = monitors.iter().map(|m| take_stills(&dir, m.name.clone()));
    let stills = futures::future::join_all(takes)
        .await
        .into_iter()
        .flatten()
        .collect();
    let monitors = monitors
        .into_iter()
        .map(|m| Monitor {
            name: m.name,
            x: m.x as f32,
            y: m.y as f32,
            focused: m.focused,
        })
        .collect();
    Snapshot {
        monitors,
        windows,
        stills,
    }
}

/// Both stills of monitor `name`, decoded for drawing.
async fn take_stills(dir: &Path, name: String) -> Option<(String, Still)> {
    let plain_path = dir.join(format!("{name}.ppm"));
    let pointer_path = dir.join(format!("{name}-pointer.ppm"));
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        eprintln!("screenshot: {}: {e}", dir.display());
        return None;
    }
    // PPM: no compression to wait for, and trivial to read back.
    let grim = |pointer: bool, path: &Path| {
        let mut command = tokio::process::Command::new("grim");
        if pointer {
            command.arg("-c");
        }
        command
            .args(["-t", "ppm", "-o", &name])
            .arg(path)
            .stdout(Stdio::null())
            .status()
    };
    let (plain, pointer) = tokio::join!(grim(false, &plain_path), grim(true, &pointer_path));
    for status in [plain, pointer] {
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("screenshot: grim -o {name}: {s}");
                return None;
            }
            Err(e) => {
                eprintln!("screenshot: grim: {e}");
                return None;
            }
        }
    }
    let decode = |path: PathBuf| tokio::task::spawn_blocking(move || load_bgra(&path));
    let (plain, pointer) = tokio::join!(decode(plain_path.clone()), decode(pointer_path.clone()));
    let (Ok(Some((width, plain))), Ok(Some((_, pointer)))) = (plain, pointer) else {
        eprintln!("screenshot: reading the stills of {name}");
        return None;
    };
    Some((
        name,
        Still {
            plain,
            pointer,
            plain_path,
            pointer_path,
            width,
        },
    ))
}

/// A PPM still as an image gpui can draw, with its width.
fn load_bgra(path: &Path) -> Option<(u32, Arc<RenderImage>)> {
    let bytes = std::fs::read(path).ok()?;
    let (width, height, rgb) = parse_ppm(&bytes)?;
    // gpui wants BGRA.
    let mut bgra = Vec::with_capacity(rgb.len() / 3 * 4);
    for px in rgb.as_chunks::<3>().0 {
        bgra.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    let buffer = ::image::RgbaImage::from_raw(width, height, bgra)?;
    Some((
        width,
        Arc::new(RenderImage::new(vec![::image::Frame::new(buffer)])),
    ))
}

/// A binary PPM (P6, 8 bits) as `(width, height, RGB bytes)`, as grim writes them.
pub fn parse_ppm(bytes: &[u8]) -> Option<(u32, u32, &[u8])> {
    let rest = bytes.strip_prefix(b"P6")?;
    let mut fields = [0u32; 3];
    let mut at = 0;
    for field in &mut fields {
        // Whitespace and comments before each number.
        loop {
            match rest.get(at)? {
                c if c.is_ascii_whitespace() => at += 1,
                b'#' => at += rest[at..].iter().position(|&c| c == b'\n')?,
                _ => break,
            }
        }
        let digits = rest[at..].iter().take_while(|c| c.is_ascii_digit()).count();
        *field = std::str::from_utf8(&rest[at..at + digits])
            .ok()?
            .parse()
            .ok()?;
        at += digits;
    }
    let [width, height, max] = fields;
    // A single whitespace byte separates the header from the pixels.
    if max != 255 || !rest.get(at)?.is_ascii_whitespace() {
        return None;
    }
    let pixels = rest.get(at + 1..)?;
    let len = width as usize * height as usize * 3;
    (pixels.len() >= len).then(|| (width, height, &pixels[..len]))
}

/// Cuts `(x, y, w, h)` (physical pixels) out of the still at `still` into a PNG at `file`, and
/// copies it to the clipboard.
pub async fn save(still: PathBuf, crop: (u32, u32, u32, u32), file: PathBuf) -> Result {
    let target = file.clone();
    tokio::task::spawn_blocking(move || -> Result {
        let bytes = std::fs::read(&still)?;
        let (width, height, rgb) = parse_ppm(&bytes).ok_or("unreadable still")?;
        let image =
            ::image::RgbImage::from_raw(width, height, rgb.to_vec()).ok_or("short still")?;
        let (x, y, w, h) = crop;
        let cut = ::image::imageops::crop_imm(&image, x, y, w, h).to_image();
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)?;
        }
        cut.save(&target)?;
        Ok(())
    })
    .await??;
    let png = std::fs::File::open(&file)?;
    let status = tokio::process::Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(png)
        .status()
        .await?;
    if !status.success() {
        return Err(format!("wl-copy: {status}").into());
    }
    Ok(())
}

/// Removes this opening's stills.
pub async fn forget_stills(opening: u64) {
    let _ = tokio::fs::remove_dir_all(stills_dir(opening)).await;
}

/// notify-send's arguments for the notification about `file`: its thumbnail (or a video icon),
/// a click to edit or open it, and actions for the rest.
pub fn notification_args(file: &Path, video: bool) -> Vec<String> {
    let path = file.to_string_lossy().into_owned();
    let name = file
        .file_name()
        .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
    let mut args: Vec<String> = vec!["--wait".into(), "-a".into()];
    if video {
        args.extend(
            [
                "Screen recording",
                "-i",
                "video-x-generic",
                "-A",
                "default=Open",
                "-A",
                "folder=Show in Files",
            ]
            .map(String::from),
        );
    } else {
        args.extend(["Screenshot".into(), "-i".into(), path.clone()]);
        args.extend(["-h".into(), format!("string:image-path:{path}")]);
        args.extend(["-A", "default=Edit", "-A", "ocr=Copy text"].map(String::from));
    }
    args.extend(["-A", "shelf=Add to shelf"].map(String::from));
    if video {
        args.extend(["Screen recording saved".into(), name]);
    } else {
        args.extend(
            [
                "Screenshot captured",
                "Copied to the clipboard · click to edit",
            ]
            .map(String::from),
        );
    }
    args
}

/// Shows the notification about `file` and waits for it to go; the action picked, if any.
pub async fn notify(file: PathBuf, video: bool) -> Option<String> {
    let output = tokio::process::Command::new("notify-send")
        .args(notification_args(&file, video))
        .output()
        .await;
    match output {
        Ok(o) => {
            let action = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (!action.is_empty()).then_some(action)
        }
        Err(e) => {
            eprintln!("screenshot: notify-send: {e}");
            None
        }
    }
}

/// A one-off notification: a failure, or what "Copy text" did.
pub async fn note(title: String, body: String) {
    super::spawn("notify-send", &["-a", "Screenshot", &title, &body]).await;
}

/// The command for notification action `action` on `file`, if it runs one ("shelf" and "ocr"
/// are done in the shell).
pub fn action_command(action: &str, file: &Path, video: bool) -> Option<Vec<String>> {
    let path = file.to_string_lossy().into_owned();
    let command: Vec<String> = match action {
        "default" if video => ["uwsm-app", "--", "xdg-open", &path]
            .map(String::from)
            .to_vec(),
        "default" => vec![
            "uwsm-app".into(),
            "--".into(),
            "gradia".into(),
            format!("--screenshot-file={path}"),
        ],
        "folder" => vec![
            "dbus-send".into(),
            "--session".into(),
            "--dest=org.freedesktop.FileManager1".into(),
            "--type=method_call".into(),
            "/org/freedesktop/FileManager1".into(),
            "org.freedesktop.FileManager1.ShowItems".into(),
            format!("array:string:file://{path}"),
            "string:".into(),
        ],
        _ => return None,
    };
    Some(command)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HyprMonitor {
    id: i64,
    name: String,
    x: i32,
    y: i32,
    focused: bool,
    active_workspace: WorkspaceRef,
    special_workspace: WorkspaceRef,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Client {
    at: [i32; 2],
    size: [i32; 2],
    mapped: bool,
    hidden: bool,
    workspace: WorkspaceRef,
    #[serde(rename = "focusHistoryID")]
    focus_history_id: i64,
}

/// No special workspace shows up as one without an id.
#[derive(Deserialize)]
struct WorkspaceRef {
    id: Option<i32>,
}

/// The mapped windows on each monitor's shown workspaces (normal and special), by focus history,
/// which is close to the stacking order for tiled and floating windows alike.
fn shown_windows(monitors: &[HyprMonitor], clients: &[Client]) -> Vec<Rect> {
    let shown: Vec<i32> = monitors
        .iter()
        .filter(|m| m.id >= 0)
        .flat_map(|m| [m.active_workspace.id, m.special_workspace.id])
        .flatten()
        .filter(|&id| id != 0)
        .collect();
    let mut windows: Vec<&Client> = clients
        .iter()
        .filter(|c| c.mapped && !c.hidden)
        .filter(|c| c.workspace.id.is_some_and(|id| shown.contains(&id)))
        .collect();
    windows.sort_by_key(|c| c.focus_history_id);
    windows
        .into_iter()
        .map(|c| {
            Rect::new(
                c.at[0] as f32,
                c.at[1] as f32,
                c.size[0] as f32,
                c.size[1] as f32,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_grim_ppm() {
        let mut ppm = b"P6\n2 1\n255\n".to_vec();
        ppm.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(parse_ppm(&ppm), Some((2, 1, &[1, 2, 3, 4, 5, 6][..])));
        // Comments in the header, and a pixel that happens to be whitespace.
        let mut ppm = b"P6 # grim\n1 1 255\n".to_vec();
        ppm.extend_from_slice(&[b'\n', 0, 0]);
        assert_eq!(parse_ppm(&ppm), Some((1, 1, &[b'\n', 0, 0][..])));
        assert_eq!(parse_ppm(b"P6\n2 2\n255\n\x01\x02"), None);
        assert_eq!(parse_ppm(b"P5\n1 1\n255\n\x01"), None);
    }

    #[test]
    fn windows_on_shown_workspaces_by_focus() {
        let monitors: Vec<HyprMonitor> = serde_json::from_str(
            r#"[{"id":0,"name":"DP-2","x":2560,"y":0,"focused":false,
                 "activeWorkspace":{"id":2},"specialWorkspace":{"name":""}},
                {"id":1,"name":"DP-3","x":0,"y":0,"focused":true,
                 "activeWorkspace":{"id":6},"specialWorkspace":{"id":-98}}]"#,
        )
        .unwrap();
        let clients: Vec<Client> = serde_json::from_str(
            r#"[{"at":[0,0],"size":[10,10],"mapped":true,"hidden":false,"workspace":{"id":6},"focusHistoryID":3},
                {"at":[1,1],"size":[10,10],"mapped":true,"hidden":false,"workspace":{"id":3},"focusHistoryID":0},
                {"at":[2,2],"size":[10,10],"mapped":true,"hidden":true,"workspace":{"id":2},"focusHistoryID":1},
                {"at":[3,3],"size":[10,10],"mapped":true,"hidden":false,"workspace":{"id":-98},"focusHistoryID":2},
                {"at":[4,4],"size":[10,10],"mapped":true,"hidden":false,"workspace":{"id":2},"focusHistoryID":4}]"#,
        )
        .unwrap();
        let xs: Vec<f32> = shown_windows(&monitors, &clients)
            .iter()
            .map(|r| r.x)
            .collect();
        assert_eq!(xs, [3., 0., 4.]);
    }

    #[test]
    fn notification_for_a_screenshot() {
        let args = notification_args(Path::new("/p/Screenshot from x.png"), false);
        assert_eq!(args[..4], ["--wait", "-a", "Screenshot", "-i"]);
        assert!(args.contains(&"string:image-path:/p/Screenshot from x.png".to_string()));
        assert!(args.contains(&"ocr=Copy text".to_string()));
        assert_eq!(
            args.last().unwrap(),
            "Copied to the clipboard · click to edit"
        );
    }

    #[test]
    fn notification_for_a_recording() {
        let args = notification_args(Path::new("/v/Screencast from x.mp4"), true);
        assert!(args.contains(&"folder=Show in Files".to_string()));
        assert!(args.contains(&"shelf=Add to shelf".to_string()));
        assert_eq!(
            args[args.len() - 2..],
            ["Screen recording saved", "Screencast from x.mp4"]
        );
    }

    #[test]
    fn actions_run_the_right_tools() {
        let file = Path::new("/p/a.png");
        assert_eq!(
            action_command("default", file, false).unwrap(),
            ["uwsm-app", "--", "gradia", "--screenshot-file=/p/a.png"]
        );
        assert_eq!(
            action_command("default", file, true).unwrap()[2..],
            ["xdg-open", "/p/a.png"]
        );
        assert!(action_command("ocr", file, false).is_none());
        assert!(action_command("shelf", file, false).is_none());
    }
}
