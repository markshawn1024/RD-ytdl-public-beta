# RD Media Toolkit

基于 Tauri 2 的本地视频下载器。Windows 安装包内含 yt-dlp、ffmpeg、Deno 和 WebView2 离线安装程序。收到安装包的人不需要安装 Node.js、Rust、Python 或命令行工具。

## 给 Windows 使用者

1. 获取 `RD-Media-Toolkit-Windows-x64` 构建产物，解压后双击其中的 `*-setup.exe`。
2. 按提示完成安装，从开始菜单启动 **Racing Dispatch Media Toolkit**。
3. 粘贴视频链接，点击“解析视频”，再点击“加入队列”。默认保存位置为“下载”文件夹下的 `RD Media Toolkit`。

支持 Windows 10/11 x64。安装本身无需联网；下载视频仍需联网，且受视频站点的访问限制影响。安装包目前未签名，Windows 可能显示“未知发布者”提示。

## 构建 Windows 安装包

在 **Windows x64 构建机**安装 Node.js 20+、Rust MSVC 工具链与 Visual Studio C++ Build Tools。它们只供开发和构建使用，不需要安装在使用者电脑上。

```powershell
npm ci
npm run build:windows
```

命令会下载并验证 Windows 版 yt-dlp、ffmpeg、Deno，再生成 NSIS 安装程序。产物位于 `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`。只需将 `*-setup.exe` 发给其他人，不要单独发送 `src-tauri/target/.../release/` 目录中的主程序。

GitHub Actions 的 `Build` 工作流也会自动构建 Windows 安装包，并上传为 `RD-Media-Toolkit-Windows-x64` artifact。可从 Actions 的对应运行页面下载。

## macOS 开发

安装 Node.js 20+、Rust 和 Xcode Command Line Tools 后：

```bash
npm ci
npm run fetch-binaries
npm run tauri:dev
```

`npm run fetch-binaries` 在 Windows 调用 PowerShell，在 macOS 调用 Bash。`npm run tauri:build` 可按当前平台执行常规打包。

## 项目结构

- React / TypeScript 前端：下载、队列、历史、设置
- Tauri / Rust 后端：调用内置的 yt-dlp、ffmpeg、Deno；保存本地设置和记录
- `scripts/fetch-binaries.ps1`：Windows 构建时准备三项内置工具
- `scripts/fetch-binaries.sh`：macOS 构建时准备内置工具

请只下载有权保存的内容，并遵守相应平台的规则。
