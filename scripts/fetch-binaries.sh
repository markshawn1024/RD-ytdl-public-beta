#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${ROOT_DIR}/src-tauri/binaries"
mkdir -p "${BIN_DIR}"

detect_triple() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}" in
    Darwin)
      case "${arch}" in
        arm64) echo "aarch64-apple-darwin" ;;
        x86_64) echo "x86_64-apple-darwin" ;;
        *) echo "unsupported macOS arch: ${arch}" >&2; exit 1 ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      echo "x86_64-pc-windows-msvc"
      ;;
    Linux)
      case "${arch}" in
        x86_64) echo "x86_64-unknown-linux-gnu" ;;
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        *) echo "unsupported Linux arch: ${arch}" >&2; exit 1 ;;
      esac
      ;;
    *)
      echo "unsupported OS: ${os}" >&2
      exit 1
      ;;
  esac
}

TRIPLE="${TAURI_ENV_TARGET_TRIPLE:-$(detect_triple)}"
echo "==> Target triple: ${TRIPLE}"

download_first_ok() {
  local out="$1"
  shift
  local url
  for url in "$@"; do
    echo "Trying ${url}"
    if curl -L --fail --retry 3 --retry-delay 2 --connect-timeout 20 --max-time 600 -o "${out}.partial" "${url}"; then
      mv "${out}.partial" "${out}"
      return 0
    fi
    rm -f "${out}.partial"
  done
  return 1
}

is_real_binary() {
  local path="$1"
  [[ -f "${path}" ]] || return 1
  local size
  size="$(wc -c < "${path}" | tr -d ' ')"
  # placeholders are tiny
  [[ "${size}" -gt 1000000 ]]
}

need_fetch() {
  local path="$1"
  [[ "${FORCE:-0}" == "1" ]] && return 0
  is_real_binary "${path}" && return 1
  return 0
}

YT_OUT="${BIN_DIR}/yt-dlp-${TRIPLE}"
FF_OUT="${BIN_DIR}/ffmpeg-${TRIPLE}"
case "${TRIPLE}" in
  *-windows-*)
    YT_OUT="${YT_OUT}.exe"
    FF_OUT="${FF_OUT}.exe"
    ;;
esac

fetch_yt_dlp() {
  case "${TRIPLE}" in
    *-windows-*)
      download_first_ok "${YT_OUT}" \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
      ;;
    *-apple-darwin)
      download_first_ok "${YT_OUT}" \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
      chmod +x "${YT_OUT}"
      ;;
    *-linux-*)
      download_first_ok "${YT_OUT}" \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
      chmod +x "${YT_OUT}"
      ;;
    *)
      echo "No yt-dlp mapping for ${TRIPLE}" >&2
      exit 1
      ;;
  esac
}

fetch_ffmpeg() {
  # Prefer redistributable static builds. Do NOT copy Homebrew ffmpeg first —
  # it links against Cellar dylibs and will break when bundled into the app.
  case "${TRIPLE}" in
    aarch64-apple-darwin)
      download_first_ok "${FF_OUT}" \
        "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/ffmpeg-darwin-arm64" \
        || fetch_ffmpeg_via_npm
      chmod +x "${FF_OUT}" || true
      ;;
    x86_64-apple-darwin)
      download_first_ok "${FF_OUT}" \
        "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/ffmpeg-darwin-x64" \
        || fetch_ffmpeg_via_npm
      chmod +x "${FF_OUT}" || true
      ;;
    x86_64-pc-windows-msvc)
      download_first_ok "${FF_OUT}" \
        "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/ffmpeg-win32-x64" \
        || fetch_ffmpeg_via_npm
      ;;
    x86_64-unknown-linux-gnu)
      download_first_ok "${FF_OUT}" \
        "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/ffmpeg-linux-x64" \
        || fetch_ffmpeg_via_npm
      chmod +x "${FF_OUT}" || true
      ;;
    aarch64-unknown-linux-gnu)
      download_first_ok "${FF_OUT}" \
        "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.0/ffmpeg-linux-arm64" \
        || fetch_ffmpeg_via_npm
      chmod +x "${FF_OUT}" || true
      ;;
    *)
      echo "No ffmpeg mapping for ${TRIPLE}" >&2
      exit 1
      ;;
  esac

  if ! is_real_binary "${FF_OUT}"; then
    echo "ffmpeg binary missing or too small after fetch" >&2
    exit 1
  fi
}

fetch_ffmpeg_via_npm() {
  echo "Falling back to npm package ffmpeg-static..."
  (
    cd "${ROOT_DIR}"
    npm install --no-save ffmpeg-static >/dev/null
    node -e "const fs=require('fs');const p=require('ffmpeg-static');fs.copyFileSync(p, process.argv[1]);" "${FF_OUT}"
  )
  chmod +x "${FF_OUT}" || true
}

if need_fetch "${YT_OUT}"; then
  fetch_yt_dlp
else
  echo "yt-dlp already present: ${YT_OUT}"
fi

if need_fetch "${FF_OUT}"; then
  fetch_ffmpeg
else
  echo "ffmpeg already present: ${FF_OUT}"
fi

case "${TRIPLE}" in
  *-windows-*)
    cp -f "${YT_OUT}" "${BIN_DIR}/yt-dlp.exe"
    cp -f "${FF_OUT}" "${BIN_DIR}/ffmpeg.exe"
    ;;
  *)
    cp -f "${YT_OUT}" "${BIN_DIR}/yt-dlp"
    cp -f "${FF_OUT}" "${BIN_DIR}/ffmpeg"
    chmod +x "${BIN_DIR}/yt-dlp" "${BIN_DIR}/ffmpeg"
    ;;
esac

echo "==> Binaries ready in ${BIN_DIR}"
ls -lh "${BIN_DIR}"
