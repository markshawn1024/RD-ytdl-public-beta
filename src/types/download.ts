export type TaskStatus =
  | "queued"
  | "probing"
  | "ready"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface DownloadTask {
  id: string;
  url: string;
  title?: string | null;
  thumbnail?: string | null;
  formatId: string;
  outputDir: string;
  outputPath?: string | null;
  status: TaskStatus;
  progress: number;
  speed?: string | null;
  eta?: string | null;
  logTail: string[];
  error?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface VideoFormat {
  formatId: string;
  ext?: string | null;
  resolution?: string | null;
  fps?: number | null;
  vcodec?: string | null;
  acodec?: string | null;
  filesize?: number | null;
  formatNote?: string | null;
  tbr?: number | null;
}

export interface VideoInfo {
  id?: string | null;
  title: string;
  thumbnail?: string | null;
  duration?: number | null;
  uploader?: string | null;
  webpageUrl?: string | null;
  formats: VideoFormat[];
}

export interface StartDownloadRequest {
  url: string;
  formatId?: string | null;
  outputDir?: string | null;
  title?: string | null;
  thumbnail?: string | null;
}

export interface AppConfig {
  downloadDir: string;
  concurrency: number;
  proxy?: string | null;
  downloadSubtitles: boolean;
  language: string;
}

export interface BinaryVersions {
  ytDlp?: string | null;
  ffmpeg?: string | null;
}

export interface ProgressEvent {
  id: string;
  percent: number;
  speed?: string | null;
  eta?: string | null;
}

export interface LogEvent {
  id: string;
  line: string;
}

export interface AppError {
  code: string;
  message: string;
}

export function formatBytes(n?: number | null): string {
  if (!n || n <= 0) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let value = n;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDuration(seconds?: number | null): string {
  if (!seconds || seconds <= 0) return "-";
  const s = Math.floor(seconds % 60);
  const m = Math.floor((seconds / 60) % 60);
  const h = Math.floor(seconds / 3600);
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function statusLabel(status: TaskStatus): string {
  switch (status) {
    case "queued":
      return "排队中";
    case "probing":
      return "解析中";
    case "ready":
      return "就绪";
    case "running":
      return "下载中";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "cancelled":
      return "已取消";
  }
}
