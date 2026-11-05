//! Commands from outside: `shell msg <command>` writes a line to a socket the running shell
//! listens on. Hyprland binds use it in place of Quickshell's global shortcuts.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use futures::SinkExt;
use futures::channel::mpsc::Sender;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use super::Result;

/// `SHELL_SOCKET` overrides it, so a development copy can run beside the real shell.
fn socket() -> PathBuf {
    if let Some(path) = std::env::var_os("SHELL_SOCKET") {
        return path.into();
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into());
    PathBuf::from(runtime).join("shell.sock")
}

/// Each command received, as its text ("session", "shelf", ...).
pub fn watch() -> futures::channel::mpsc::Receiver<String> {
    crate::runtime::stream(8, async |out| {
        super::supervise("ipc", Duration::from_secs(5), out, serve).await
    })
}

async fn serve(mut out: Sender<String>) -> Result {
    let path = socket();
    // Left behind by a shell that didn't exit cleanly; one that's running answers instead.
    if std::os::unix::net::UnixStream::connect(&path).is_err() {
        let _ = std::fs::remove_file(&path);
    }
    let listener = UnixListener::bind(&path)?;
    loop {
        let (stream, _) = listener.accept().await?;
        let (read, mut write) = stream.into_split();
        let mut line = String::new();
        BufReader::new(read).read_line(&mut line).await?;
        out.send(line.trim().to_string()).await?;
        write.write_all(b"ok\n").await?;
    }
}

/// Whether a shell is already listening, so a second one doesn't start (and double every bar).
pub fn running() -> bool {
    std::os::unix::net::UnixStream::connect(socket()).is_ok()
}

/// The client side, for `shell msg <command>`. Prints the shell's reply.
pub fn send(command: &str) -> std::io::Result<()> {
    let mut stream = std::os::unix::net::UnixStream::connect(socket())?;
    stream.write_all(format!("{command}\n").as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    print!("{reply}");
    Ok(())
}
