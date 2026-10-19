//! Images: ImageMagick for pixels, exiftool for metadata, tesseract for text.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use anyhow::bail;

use super::{Outcome, exec, output};
use crate::{
    actions::{CONVERT_FORMATS, RESIZE_SIZES},
    files::{ext, out_dir, plural, stem, unique},
};

/// The input as ImageMagick should read it: only the first frame of an animated GIF,
/// so converting it gives one picture rather than one per frame.
fn magick_input(file: &Path, target: &str) -> OsString {
    let mut arg = file.as_os_str().to_owned();
    if ext(file) == "gif" && target != "gif" {
        arg.push("[0]");
    }
    arg
}

pub async fn convert(files: &[PathBuf], format: &str) -> anyhow::Result<Outcome> {
    let Some(&(format, label)) = CONVERT_FORMATS.iter().find(|(id, _)| *id == format) else {
        bail!("Pick a format to convert to");
    };
    let dir = out_dir()?;
    let mut made = Vec::new();
    for file in files {
        let out = unique(&dir, &format!("{}.{format}", stem(file)));
        exec("magick", [magick_input(file, format), out.clone().into()]).await?;
        made.push(out);
    }
    Ok(Outcome::made(
        format!("Converted {} to {label}", plural(files.len(), "image")),
        made,
    ))
}

pub async fn resize(files: &[PathBuf], size: &str) -> anyhow::Result<Outcome> {
    let Some(&(size, label)) = RESIZE_SIZES.iter().find(|(id, _)| *id == size) else {
        bail!("Pick a size to resize to");
    };
    // Percentages scale; pixel sizes set the width, never enlarging.
    let (geometry, suffix) = match size.strip_suffix('%') {
        Some(pct) => (size.to_string(), format!("{pct}pct")),
        None => (format!("{size}x>"), format!("{size}w")),
    };
    let dir = out_dir()?;
    let mut made = Vec::new();
    for file in files {
        let out = unique(&dir, &format!("{}-{suffix}.{}", stem(file), ext(file)));
        let target = ext(file);
        exec(
            "magick",
            [
                magick_input(file, &target),
                "-resize".into(),
                geometry.clone().into(),
                out.clone().into(),
            ],
        )
        .await?;
        made.push(out);
    }
    Ok(Outcome::made(
        format!("Resized {} to {label}", plural(files.len(), "image")),
        made,
    ))
}

pub async fn strip(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = out_dir()?;
    let mut made = Vec::new();
    for file in files {
        let out = unique(&dir, &format!("{}-clean.{}", stem(file), ext(file)));
        // Rewrites the metadata only, so the pixels aren't re-encoded.
        exec(
            "exiftool",
            [
                "-q".into(),
                "-all=".into(),
                "-o".into(),
                OsString::from(&out),
                file.into(),
            ],
        )
        .await?;
        made.push(out);
    }
    Ok(Outcome::made("Removed EXIF and GPS data", made))
}

pub async fn ocr(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let mut texts = Vec::new();
    for file in files {
        let text = output(
            "tesseract",
            [
                file.as_os_str(),
                "-".as_ref(),
                "-l".as_ref(),
                "eng+swe".as_ref(),
            ],
        )
        .await?;
        let text = text.trim();
        if !text.is_empty() {
            texts.push(text.to_string());
        }
    }
    if texts.is_empty() {
        return Ok(Outcome::failed("No text found"));
    }
    Ok(Outcome {
        copy: Some(texts.join("\n\n")),
        ..Outcome::said(format!(
            "Copied text from {} to the clipboard",
            plural(files.len(), "image")
        ))
    })
}
