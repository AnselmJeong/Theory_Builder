//! Operating-system integration kept in one place so the engine builds everywhere.
use anyhow::{Context, Result};
use std::{ffi::OsStr, path::PathBuf};

/// Per-user application data directory. `THEORY_STUDIO_DATA_DIR` overrides it.
pub fn data_directory() -> PathBuf {
    if let Some(dir) = std::env::var_os("THEORY_STUDIO_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Theory Studio")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("theory-studio")
    }
}

/// Hand a file path or a validated public URL to the desktop's default handler.
pub fn open_external(target: impl AsRef<OsStr>) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "/usr/bin/open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(opener)
        .arg(target.as_ref())
        .spawn()
        .with_context(|| format!("{opener}로 원문을 열 수 없습니다"))?;
    Ok(())
}
