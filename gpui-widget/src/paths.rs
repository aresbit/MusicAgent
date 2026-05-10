//! Portable path resolution for MusicAgent.
//!
//! All runtime file paths derive from `project_root()`, which auto-detects
//! the project root directory regardless of whether the binary is running
//! under `cargo run` (development) or as a standalone release executable.

use std::path::PathBuf;

/// Determine the project root directory.
///
/// Detection strategy:
/// 1. Cargo build layout (dev): binary at `gpui-widget/target/{debug,release}/musicagent-widget.exe`
///    → walk up 3 levels to reach the project root.
/// 2. Standalone release (packaged): binary placed at the project root alongside `packages/`.
fn project_root() -> PathBuf {
    let exe = std::env::current_exe().expect("Cannot determine executable path");
    let exe_dir = exe.parent().expect("Cannot determine executable directory");

    // Walk up and check for the canonical "packages" directory.
    let mut current = Some(exe_dir);
    while let Some(dir) = current {
        if dir.join("packages").is_dir() && dir.join("gpui-widget").is_dir() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }

    // Fallback: use the directory containing the executable.
    exe_dir.to_path_buf()
}

/// Directory of the gpui-widget crate (logs, music cache, etc.).
pub fn widget_dir() -> PathBuf {
    let root = project_root();
    let candidate = root.join("gpui-widget");
    if candidate.is_dir() {
        candidate
    } else {
        // Running from within gpui-widget already
        root
    }
}

/// Path to the opencc CLI project directory (`packages/opencc/`).
pub fn opencc_dir() -> PathBuf {
    project_root().join("packages").join("opencc")
}

/// Path to the TTS server directory (`packages/tts-server/`).
pub fn tts_server_dir() -> PathBuf {
    project_root().join("packages").join("tts-server")
}

/// Log file path inside widget_dir.
pub fn log_file(name: &str) -> PathBuf {
    widget_dir().join(format!("musicagent-{}.log", name))
}

/// Music cache directory inside widget_dir.
pub fn music_cache_dir() -> PathBuf {
    widget_dir().join("music_cache")
}
