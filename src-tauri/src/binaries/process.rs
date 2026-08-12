//! Cross-platform process helpers for sidecar binaries.

use tokio::process::Command;

/// Configure a child process so console apps (yt-dlp / ffmpeg) do not flash a
/// black CMD window under a Windows GUI host.
///
/// On Windows this sets `CREATE_NO_WINDOW` (0x08000000). stdout/stderr piping
/// remains unaffected. No-op on macOS / Linux.
pub fn configure_hidden_process(cmd: &mut Command) {
    #[cfg(windows)]
    {
        // CREATE_NO_WINDOW — hide console without CREATE_NEW_CONSOLE.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd;
}
