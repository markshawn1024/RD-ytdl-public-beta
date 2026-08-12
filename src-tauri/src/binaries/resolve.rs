use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const YT_DLP_NAME: &str = "yt-dlp";
const FFMPEG_NAME: &str = "ffmpeg";

#[derive(Debug, Clone)]
pub struct BinaryPaths {
    pub yt_dlp: PathBuf,
    pub ffmpeg: PathBuf,
}

pub fn resolve(app: &AppHandle) -> AppResult<BinaryPaths> {
    Ok(BinaryPaths {
        yt_dlp: resolve_sidecar(app, YT_DLP_NAME)?,
        ffmpeg: resolve_sidecar(app, FFMPEG_NAME)?,
    })
}

fn resolve_sidecar(app: &AppHandle, name: &str) -> AppResult<PathBuf> {
    let exe_name = binary_filename(name);
    let candidates = candidate_paths(app, &exe_name);

    for path in candidates {
        if path.is_file() {
            ensure_executable(&path)?;
            return Ok(path);
        }
    }

    Err(AppError::BinaryMissing(format!(
        "未找到 {name}，请先运行 npm run fetch-binaries"
    )))
}

fn binary_filename(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn candidate_paths(app: &AppHandle, exe_name: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join(exe_name));
            // macOS .app: Contents/MacOS
            paths.push(dir.join(exe_name));
        }
    }

    if let Ok(resource_dir) = app.path().resource_dir() {
        paths.push(resource_dir.join(exe_name));
        paths.push(resource_dir.join("binaries").join(exe_name));
    }

    // Dev / source tree layout: src-tauri/binaries/<name>-<triple>
    if let Ok(cwd) = std::env::current_dir() {
        let triple = host_target_triple();
        let with_triple = if cfg!(windows) {
            format!("{}-{}.exe", exe_name.trim_end_matches(".exe"), triple)
        } else {
            format!("{exe_name}-{triple}")
        };

        for base in [
            cwd.join("binaries"),
            cwd.join("src-tauri").join("binaries"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries"),
        ] {
            paths.push(base.join(exe_name));
            paths.push(base.join(&with_triple));
        }
    }

    paths
}

fn host_target_triple() -> String {
    if let Ok(triple) = std::env::var("TAURI_ENV_TARGET_TRIPLE") {
        return triple;
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return "aarch64-apple-darwin".into();
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return "x86_64-apple-darwin".into();
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return "x86_64-pc-windows-msvc".into();
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        return "aarch64-pc-windows-msvc".into();
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return "x86_64-unknown-linux-gnu".into();
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return "aarch64-unknown-linux-gnu".into();
    }
    #[allow(unreachable_code)]
    "unknown".into()
}

fn ensure_executable(path: &Path) -> AppResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(path)?;
        let mut perms = meta.permissions();
        let mode = perms.mode();
        if mode & 0o111 == 0 {
            perms.set_mode(mode | 0o755);
            std::fs::set_permissions(path, perms)?;
        }
    }
    let _ = path;
    Ok(())
}

pub async fn read_version(bin: &Path) -> Option<String> {
    // Prefer --version (yt-dlp); fall back to -version (ffmpeg).
    for arg in ["--version", "-version"] {
        let mut cmd = tokio::process::Command::new(bin);
        cmd.arg(arg);
        crate::binaries::configure_hidden_process(&mut cmd);
        if let Ok(output) = cmd.output().await {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let line = text.lines().next().unwrap_or("").trim();
                if !line.is_empty() {
                    return Some(line.to_string());
                }
            }
        }
    }
    None
}
