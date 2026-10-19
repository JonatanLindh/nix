//! A one-file HTTP server on localhost for funnel links. Tailscale only lets root serve
//! paths, but any operator may proxy to a local port, so the file is served from here.

use std::path::{Path, PathBuf};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::files::{ext, name};

/// Starts serving `file` on 127.0.0.1 (any path, GET or HEAD) until `stop` fires.
/// Returns the port.
pub async fn serve(file: PathBuf, stop: oneshot::Receiver<()>) -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tokio::spawn(async move {
        let accept = async {
            while let Ok((stream, _)) = listener.accept().await {
                let file = file.clone();
                tokio::spawn(async move {
                    if let Err(e) = respond(stream, &file).await {
                        eprintln!("funnel request: {e}");
                    }
                });
            }
        };
        tokio::select! {
            _ = accept => {}
            _ = stop => {}
        }
    });
    Ok(port)
}

async fn respond(mut stream: TcpStream, file: &Path) -> std::io::Result<()> {
    // Only the request line matters; read the head (bounded) and ignore the rest.
    let mut head = Vec::new();
    let mut buf = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 16 * 1024 {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        head.extend_from_slice(&buf[..n]);
    }
    let method = head.split(|&b| b == b' ').next().unwrap_or_default();
    if method != b"GET" && method != b"HEAD" {
        return stream
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
    }

    let mut body = match tokio::fs::File::open(file).await {
        Ok(f) => f,
        Err(_) => {
            return stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
        }
    };
    let len = body.metadata().await?.len();
    // Quotes would end the header value; the URL already carries a clean name anyway.
    let filename = name(file).replace(['"', '\\', '\r', '\n'], "_");
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {len}\r\n\
         Content-Disposition: inline; filename=\"{filename}\"\r\nConnection: close\r\n\r\n",
        content_type(file)
    );
    stream.write_all(header.as_bytes()).await?;
    if method == b"GET" {
        tokio::io::copy(&mut body, &mut stream).await?;
    }
    stream.shutdown().await
}

fn content_type(file: &Path) -> &'static str {
    match ext(file).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "txt" | "md" => "text/plain; charset=utf-8",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}
