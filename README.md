# RD Media Toolkit

跨平台本地视频下载与素材工具（第一阶段：基于内置 yt-dlp + ffmpeg 的下载器）。

## 技术栈

- Tauri 2 / Rust
- React + TypeScript
- Tailwind CSS
- Zustand

## 快速开始

1. 安装 [Rust](https://www.rust-lang.org/tools/install) 与 Node.js 20+
2. macOS 需 Xcode CLT；Windows 需 WebView2

```bash
npm install
npm run fetch-binaries   # 下载当前平台的 yt-dlp / ffmpeg 到 src-tauri/binaries/
npm run tauri:dev
```

## 常用命令

| 命令 | 说明 |
|---|---|
| `npm run tauri:dev` | 开发模式 |
| `npm run tauri:build` | 打包当前平台 |
| `npm run fetch-binaries` | 拉取内置 sidecar |
| `FORCE=1 npm run fetch-binaries` | 强制重新下载 |

## 架构

- **前端**：下载 / 历史 / 设置三页；订阅 `download://*` 事件
- **Rust**：`TaskManager` 队列与并发；`runner` 调用内置 yt-dlp；进度经 Event 推送
- **二进制**：Tauri `externalBin` sidecar（`binaries/yt-dlp`、`binaries/ffmpeg`）
- **无服务器**；设置与历史为本地 JSON

## 打包

```bash
npm run fetch-binaries
npm run tauri:build
```

产物目录：`src-tauri/target/release/bundle/`

CI：见 [`.github/workflows/build.yml`](.github/workflows/build.yml)（macOS arm64 / macOS x64 / Windows x64）。
