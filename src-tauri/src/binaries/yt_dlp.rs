//! Shared yt-dlp CLI arguments (bundled Deno JS runtime, EJS, cache dir).

use crate::binaries::BinaryPaths;
use crate::error::{AppError, AppResult};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::process::Command;

/// Append bundled Deno, EJS remote component, and cache-dir to a yt-dlp command.
pub fn append_runtime_args(cmd: &mut Command, bins: &BinaryPaths, cache_dir: &Path) {
    let deno = bundled_deno_location(&bins.deno);
    cmd.arg("--js-runtimes")
        .arg(format!("deno:{}", deno.display()))
        .arg("--remote-components")
        .arg("ejs:github")
        .arg("--cache-dir")
        .arg(cache_dir);
}

/// Writable yt-dlp cache under app data; seed from bundled resources when empty.
pub fn cache_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| AppError::Message(e.to_string()))?
        .join("yt-dlp");
    fs::create_dir_all(&dir)?;

    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("yt-dlp-cache");
        if bundled.is_dir() {
            let _ = seed_cache_dir(&bundled, &dir);
        }
    }

    // Dev / source tree: resources next to manifest.
    let dev_bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/yt-dlp-cache");
    if dev_bundled.is_dir() {
        let _ = seed_cache_dir(&dev_bundled, &dir);
    }

    Ok(dir)
}

fn seed_cache_dir(from: &Path, to: &Path) -> AppResult<()> {
    if !from.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let name = entry.file_name();
        let dst = to.join(name);
        if src.is_dir() {
            if !dst.exists() {
                copy_dir_recursive(&src, &dst)?;
            }
        } else if !dst.exists() {
            fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> AppResult<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if !to.exists() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Absolute path to bundled Deno for `--js-runtimes deno:PATH`.
pub fn bundled_deno_location(deno: &Path) -> PathBuf {
    let abs = fs::canonicalize(deno).unwrap_or_else(|_| deno.to_path_buf());
    normalize_path_for_yt_dlp(abs)
}

fn normalize_path_for_yt_dlp(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

pub fn is_js_runtime_missing(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("no supported javascript runtime")
        || lower.contains("javascript runtime could not be found")
        || lower.contains("only deno is enabled by default")
}

/// User-facing failure message; HTTP 403 is not treated as network when JS runtime is missing.
pub fn format_failure_message(detail: &str, saw_runtime_warning: bool) -> String {
    if is_js_runtime_missing(detail) || saw_runtime_warning {
        return format!(
            "YouTube 解析环境异常\n内置 JavaScript Runtime 不可用，请更新或重新安装应用。\n\n原始错误:\n{detail}"
        );
    }

    let lower = detail.to_ascii_lowercase();
    if lower.contains("unable to download")
        || lower.contains("network")
        || lower.contains("urlopen")
        || lower.contains("timed out")
        || (lower.contains("http error 403") && !saw_runtime_warning)
    {
        return format!("网络错误: {detail}");
    }

    format!("下载失败: {detail}")
}

pub fn format_probe_failure(detail: &str, saw_runtime_warning: bool) -> String {
    if is_js_runtime_missing(detail) || saw_runtime_warning {
        return format!(
            "YouTube 解析环境异常：内置 JavaScript Runtime 不可用，请更新或重新安装应用。\n\n原始错误:\n{detail}"
        );
    }
    detail.to_string()
}
