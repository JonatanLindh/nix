//! 1Password through the `op` CLI and the desktop app integration, as Passwords.qml had it: the
//! data behind the launcher's passwords mode. Ported from `qs-op` (scripts/ in ~/nix), which the
//! shell used to run for this.
//!
//! [`list`] and [`fields`] give metadata only: concealed and one-time-code values come back as
//! `None`. [`take`] fetches a value and puts it straight onto the clipboard: through wl-copy's
//! stdin (never its arguments), marked sensitive so cliphist skips it, and cleared after 30s
//! unless something else has been copied since. Only a hash of it is kept for that. Buffers that
//! held a secret are zeroed when dropped.
//!
//! The clear is a transient systemd timer, so it survives the shell stopping or restarting (a
//! child process wouldn't: stopping the shell's service kills its whole cgroup). Without a user
//! manager it falls back to a task on the services runtime.

use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use zeroize::{Zeroize, Zeroizing};

use super::clip::wl_copy;

/// How long a copied secret stays on the clipboard.
const CLEAR_AFTER: Duration = Duration::from_secs(30);

/// An item's metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub title: String,
    /// "LOGIN", "SECURE_NOTE", "API_CREDENTIAL", ...
    pub category: String,
    /// The vault's id.
    pub vault: String,
    pub vault_name: String,
    pub user: String,
    /// Space-separated.
    pub urls: String,
}

/// One of an item's fields; `value` is `None` for secrets.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub id: String,
    pub label: String,
    /// "STRING", "CONCEALED", "OTP", "URL", ...
    pub kind: String,
    pub value: Option<String>,
}

impl Field {
    pub fn is_otp(&self) -> bool {
        self.kind == "OTP"
    }
}

// ---- What `op` prints

/// An entry of `op item list --format json`.
#[derive(Deserialize)]
struct OpItem {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    category: Option<String>,
    vault: OpVault,
    #[serde(default)]
    additional_information: Option<String>,
    #[serde(default)]
    urls: Option<Vec<OpUrl>>,
}

#[derive(Deserialize)]
struct OpVault {
    id: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
struct OpUrl {
    #[serde(default)]
    href: Option<String>,
}

/// `op item get --format json`, of which only the fields matter.
#[derive(Deserialize)]
struct OpDetails {
    #[serde(default)]
    fields: Option<Vec<OpField>>,
}

#[derive(Deserialize)]
struct OpField {
    id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    /// A secret, for concealed fields: zeroed as soon as the fields are read.
    #[serde(default)]
    value: Option<serde_json::Value>,
}

impl Drop for OpField {
    fn drop(&mut self) {
        if let Some(serde_json::Value::String(value)) = &mut self.value {
            value.zeroize();
        }
    }
}

/// The items in `op item list --format json`.
fn parse_items(json: &[u8]) -> Option<Vec<Item>> {
    let items: Vec<OpItem> = serde_json::from_slice(json).ok()?;
    let items = items.into_iter().map(|i| Item {
        id: i.id,
        title: i.title.unwrap_or_default(),
        category: i.category.unwrap_or_default(),
        vault: i.vault.id,
        vault_name: i.vault.name.unwrap_or_default(),
        user: i.additional_information.unwrap_or_default(),
        urls: (i.urls.unwrap_or_default().into_iter())
            .map(|u| u.href.unwrap_or_default())
            .collect::<Vec<_>>()
            .join(" "),
    });
    Some(items.collect())
}

/// The fields in `op item get --format json` worth showing: those with a value, and one-time
/// codes (whose value op keeps to itself). Secret values are left out.
fn parse_fields(json: &[u8]) -> Option<Vec<Field>> {
    let details: OpDetails = serde_json::from_slice(json).ok()?;
    let fields = details.fields.unwrap_or_default();
    let fields = fields.iter().filter_map(|f| {
        let kind = f.kind.clone().unwrap_or_default();
        let value = match &f.value {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) if s.is_empty() => None,
            Some(value) => Some(value),
        };
        if kind != "OTP" && value.is_none() {
            return None;
        }
        // Secrets stay where they are.
        let secret = kind == "CONCEALED" || kind == "OTP";
        let value = value.filter(|_| !secret).map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        });
        Some(Field {
            id: f.id.clone(),
            label: f.label.clone().unwrap_or_default(),
            value,
            kind,
        })
    });
    Some(fields.collect())
}

/// The last line of what a command said on stderr, as its error.
fn last_line(stderr: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stderr);
    let text = text.trim();
    (!text.is_empty()).then(|| text.lines().last().unwrap_or_default().to_string())
}

// ---- Running `op`

/// What a run of `op` printed. Its stdout may be a secret.
struct Output {
    status: ExitStatus,
    stdout: Zeroizing<Vec<u8>>,
    stderr: Vec<u8>,
}

/// Runs `op ARGS`. Its output is read until op exits (and a moment after), not to EOF, in case
/// something it starts in the background holds on to the pipes.
async fn op(args: &[&str]) -> std::io::Result<Output> {
    let mut child = tokio::process::Command::new("op")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(std::io::Error::other("op: no pipes"));
    };
    let mut out = Zeroizing::new(Vec::with_capacity(16 * 1024));
    let mut err = Vec::new();
    let status = {
        let reading = async {
            let _ = tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        };
        tokio::pin!(reading);
        tokio::select! {
            () = &mut reading => child.wait().await?,
            status = child.wait() => {
                // What it wrote before exiting is already in the pipes.
                let _ = tokio::time::timeout(Duration::from_millis(100), &mut reading).await;
                status?
            }
        }
    };
    Ok(Output {
        status,
        stdout: out,
        stderr: err,
    })
}

/// Runs `op` for what `parse` makes of its output, and what went wrong (its last word on stderr).
async fn query<T>(args: &[&str], parse: fn(&[u8]) -> Option<T>) -> (Option<T>, Option<String>) {
    match op(args).await {
        Ok(o) => (parse(&o.stdout), last_line(&o.stderr)),
        Err(e) => (None, Some(format!("op: {e}"))),
    }
}

/// Every item, by title. Empty when it fails (or isn't authorized), with why.
pub async fn list() -> (Vec<Item>, Option<String>) {
    let (items, error) = query(&["item", "list", "--format", "json"], parse_items).await;
    let mut items = items.unwrap_or_default();
    items.sort_by_cached_key(|i| i.title.to_lowercase());
    (items, error)
}

/// An item's fields, without secret values.
pub async fn fields(vault: String, id: String) -> (Vec<Field>, Option<String>) {
    let args = ["item", "get", &id, "--vault", &vault, "--format", "json"];
    let (fields, error) = query(&args, parse_fields).await;
    (fields.unwrap_or_default(), error)
}

// ---- Copying

/// `what` of the item, as op prints it: a one-time code, or a field's value. Errors never carry
/// the value.
async fn secret(vault: &str, id: &str, what: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let reference;
    let args: &[&str] = if what == "otp" {
        &["item", "get", id, "--vault", vault, "--otp"]
    } else {
        reference = format!("op://{vault}/{id}/{what}");
        &["read", "--no-newline", &reference]
    };
    let output = op(args).await.map_err(|e| format!("op: {e}"))?;
    if !output.status.success() {
        return Err(last_line(&output.stderr).unwrap_or_else(|| format!("op: {}", output.status)));
    }
    let mut value = output.stdout;
    // As the shell's `$(...)` had it: no trailing newlines.
    let end = value.iter().rposition(|&b| b != b'\n').map_or(0, |i| i + 1);
    value[end..].zeroize();
    value.truncate(end);
    Ok(value)
}

fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// The clear's check, for `sh -c`: `$1` is the hash, in hex as `sha256sum` prints it.
const CLEAR_SCRIPT: &str = r#"[ "$(wl-paste -n 2>/dev/null | sha256sum | cut -d' ' -f1)" = "$1" ] && wl-copy --clear"#;

/// Clears the clipboard after a while, if it still holds what hashes to `hash`. Only the hash
/// leaves the shell (in the timer's arguments), never the secret.
fn clear_later(hash: [u8; 32]) {
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    crate::runtime::detach(async move {
        let on_active = format!("--on-active={}s", CLEAR_AFTER.as_secs());
        let timer = tokio::process::Command::new("systemd-run")
            // Timers fire within a minute of when they're due unless told otherwise.
            .args(["--user", "--quiet", "--collect", &on_active])
            .args(["--timer-property=AccuracySec=1s", "--"])
            .args(["sh", "-c", CLEAR_SCRIPT, "sh", &hex])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        if !timer.is_ok_and(|s| s.success()) {
            clear_in_process(hash).await;
        }
    });
}

/// The fallback: the same check from a task here, which dies with the shell.
async fn clear_in_process(hash: [u8; 32]) {
    tokio::time::sleep(CLEAR_AFTER).await;
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new("wl-paste")
            .arg("-n")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await;
    let Ok(Ok(output)) = output else {
        return;
    };
    let current = Zeroizing::new(output.stdout);
    if sha256(&current) == hash {
        crate::services::spawn("wl-copy", &["--clear"]).await;
    }
}

/// Copies `what` of the item, sensitive, and schedules the clear.
async fn copy(vault: &str, id: &str, what: &str) -> Result<(), String> {
    let value = secret(vault, id, what).await?;
    // Sensitive: cliphist doesn't record it.
    wl_copy(&["--sensitive"], &value)
        .await
        .map_err(|e| e.to_string())?;
    clear_later(sha256(&value));
    Ok(())
}

/// Puts `what` (a field id, "otp", or "password" / "username") of the item on the clipboard,
/// and with `paste` pastes it into the window that gets focus back. Runs after the launcher has
/// closed, so a failure goes out as a notification.
pub async fn take(vault: String, id: String, what: String, paste: bool) {
    match copy(&vault, &id, &what).await {
        Ok(()) if paste => crate::services::hyprland::paste().await,
        Ok(()) => {}
        Err(message) => {
            crate::services::spawn(
                "notify-send",
                &["-a", "1password", "couldn't copy", &message],
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items() {
        // As `op item list --format json` prints them (made up).
        let items = parse_items(
            br#"[
              {"id":"i1","title":"Mail","version":3,"vault":{"id":"v1","name":"Private"},
               "category":"LOGIN","last_edited_by":"u","created_at":"2024-01-01T00:00:00Z",
               "updated_at":"2024-01-01T00:00:00Z","additional_information":"me@example.com",
               "urls":[{"label":"website","primary":true,"href":"https://a.example"},
                       {"href":"https://b.example"}]},
              {"id":"i2","title":"Note","vault":{"id":"v2"},"category":"SECURE_NOTE"}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            items[0],
            Item {
                id: "i1".into(),
                title: "Mail".into(),
                category: "LOGIN".into(),
                vault: "v1".into(),
                vault_name: "Private".into(),
                user: "me@example.com".into(),
                urls: "https://a.example https://b.example".into(),
            }
        );
        assert_eq!(items[1].vault_name, "");
        assert_eq!(items[1].user, "");
        assert_eq!(items[1].urls, "");
        assert_eq!(parse_items(b""), None);
        assert_eq!(parse_items(b"[]"), Some(vec![]));
    }

    #[test]
    fn fields_without_secrets() {
        // As `op item get --format json` prints them (made up).
        let fields = parse_fields(
            br#"{"id":"i1","title":"Mail","vault":{"id":"v1"},"fields":[
              {"id":"username","type":"STRING","purpose":"USERNAME","label":"username","value":"me"},
              {"id":"password","type":"CONCEALED","purpose":"PASSWORD","label":"password","value":"hunter2"},
              {"id":"notesPlain","type":"STRING","purpose":"NOTES","label":"notesPlain"},
              {"id":"empty","type":"CONCEALED","label":"pin","value":""},
              {"id":"totp","type":"OTP","label":"one-time password","section":{"id":"s","label":"Security"}},
              {"id":"site","type":"URL","label":"website","value":"https://a.example","section":{"id":"s"}}
            ]}"#,
        )
        .unwrap();
        let shown: Vec<_> = fields
            .iter()
            .map(|f| (f.id.as_str(), f.kind.as_str(), f.value.as_deref()))
            .collect();
        assert_eq!(
            shown,
            [
                ("username", "STRING", Some("me")),
                ("password", "CONCEALED", None),
                ("totp", "OTP", None),
                ("site", "URL", Some("https://a.example")),
            ]
        );
        assert_eq!(fields[2].label, "one-time password");
        assert!(fields[2].is_otp());
        assert_eq!(parse_fields(br#"{"id":"i"}"#), Some(vec![]));
        assert_eq!(parse_fields(b"not json"), None);
    }

    #[test]
    fn errors() {
        assert_eq!(last_line(b""), None);
        assert_eq!(
            last_line(b"[ERROR] one\n[ERROR] two\n"),
            Some("[ERROR] two".into())
        );
    }

    #[test]
    fn hashes_like_sha256sum() {
        // printf %s abc | sha256sum
        let hex: String = sha256(b"abc").iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
