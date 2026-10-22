//! PDFs: qpdf to merge and split, Ghostscript to compress.

use std::{ffi::OsString, path::PathBuf};

use anyhow::Context;

use super::{Outcome, exec, size_of};
use crate::files::{human_size, name, plural, results_dir, stem, unique};

pub async fn merge(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let out = unique(&results_dir()?, "merged.pdf");
    let mut args: Vec<OsString> = vec!["--empty".into(), "--pages".into()];
    args.extend(files.iter().map(OsString::from));
    args.extend(["--".into(), out.clone().into()]);
    exec("qpdf", args).await?;
    let message = format!("Merged {} PDFs into {}", files.len(), name(&out));
    Ok(Outcome::made(message, vec![out]))
}

pub async fn compress(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    let (mut before, mut after) = (0, 0);
    let mut made = Vec::new();
    for file in files {
        let out = unique(&dir, &format!("{}-small.pdf", stem(file)));
        let mut target = OsString::from("-sOutputFile=");
        target.push(&out);
        exec(
            "gs",
            [
                "-sDEVICE=pdfwrite".into(),
                // 150 dpi images: readable on screen, a fraction of the size of scans.
                "-dPDFSETTINGS=/ebook".into(),
                "-dCompatibilityLevel=1.5".into(),
                "-dNOPAUSE".into(),
                "-dBATCH".into(),
                "-dQUIET".into(),
                target,
                file.into(),
            ],
        )
        .await?;
        before += size_of(file);
        after += size_of(&out);
        made.push(out);
    }
    let message = format!(
        "Compressed {} ({} → {})",
        plural(files.len(), "PDF"),
        human_size(before),
        human_size(after)
    );
    Ok(Outcome::made(message, made))
}

pub async fn split(files: &[PathBuf]) -> anyhow::Result<Outcome> {
    let dir = results_dir()?;
    let mut made = Vec::new();
    for file in files {
        let folder = unique(&dir, &format!("{}-pages", stem(file)));
        std::fs::create_dir_all(&folder)
            .with_context(|| format!("Can't create {}", folder.display()))?;
        // qpdf replaces %d with the zero-padded page number.
        let pattern = folder.join(format!("{}-%d.pdf", stem(file)));
        exec(
            "qpdf",
            ["--split-pages".into(), OsString::from(file), pattern.into()],
        )
        .await?;
        made.push(folder);
    }
    Ok(Outcome::made("Split into one PDF per page", made))
}
