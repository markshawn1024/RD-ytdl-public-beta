//! Cross-platform process helpers for sidecar binaries.

use std::process::Stdio;
use tokio::process::Command;

/// Configure a child process so console apps (yt-dlp / ffmpeg) do not flash a
/// black CMD window under a Windows GUI host, and never inherit a TTY/stdin that
/// can be disturbed by UI focus changes.
///
/// On Windows this sets `CREATE_NO_WINDOW` (0x08000000). stdout/stderr piping
/// remains unaffected. No-op flags on macOS / Linux besides stdin=null.
pub fn configure_hidden_process(cmd: &mut Command) {
    // Detach from any console stdin so WebView focus / UI I/O cannot affect the child.
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW — hide console without CREATE_NEW_CONSOLE.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
