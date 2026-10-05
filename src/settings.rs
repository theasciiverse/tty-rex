//! Choices that carry over between runs, in $XDG_CONFIG_HOME/tty-rex/
//! (or ~/.config/tty-rex/). For now just the look `V` last picked.

use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tty-rex"))
}

pub fn load_look() -> Option<String> {
    let s = std::fs::read_to_string(dir()?.join("look")).ok()?;
    Some(s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn save_look(name: &str) {
    let Some(d) = dir() else { return };
    let result = std::fs::create_dir_all(&d).and_then(|_| std::fs::write(d.join("look"), format!("{name}\n")));
    if let Err(e) = result {
        log::warn!("can't save the look to {}: {e}", d.display());
    }
}
