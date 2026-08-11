import { invoke } from "@tauri-apps/api/core";
import type {
  AppConfig,
  AppError,
  BinaryVersions,
  DownloadTask,
  StartDownloadRequest,
  VideoInfo,
} from "../types/download";

function asError(error: unknown): AppError {
  if (typeof error === "object" && error && "message" in error) {
    const e = error as AppError;
    return { code: e.code ?? "Message", message: e.message };
  }
  return { code: "Message", message: String(error) };
}

export async function probeUrl(url: string): Promise<VideoInfo> {
  try {
    return await invoke<VideoInfo>("probe_url", { url });
  } catch (error) {
    throw asError(error);
  }
}

export async function startDownload(
  request: StartDownloadRequest,
): Promise<DownloadTask> {
  try {
    return await invoke<DownloadTask>("start_download", { request });
  } catch (error) {
    throw asError(error);
  }
}

export async function cancelDownload(id: string): Promise<DownloadTask> {
  try {
    return await invoke<DownloadTask>("cancel_download", { id });
  } catch (error) {
    throw asError(error);
  }
}

export async function listDownloads(): Promise<DownloadTask[]> {
  try {
    return await invoke<DownloadTask[]>("list_downloads");
  } catch (error) {
    throw asError(error);
  }
}

export async function clearFinished(): Promise<void> {
  try {
    await invoke("clear_finished");
  } catch (error) {
    throw asError(error);
  }
}

export async function getSettings(): Promise<AppConfig> {
  try {
    return await invoke<AppConfig>("get_settings");
  } catch (error) {
    throw asError(error);
  }
}

export async function updateSettings(settings: AppConfig): Promise<AppConfig> {
  try {
    return await invoke<AppConfig>("update_settings", { settings });
  } catch (error) {
    throw asError(error);
  }
}

export async function selectDirectory(): Promise<string | null> {
  try {
    return await invoke<string | null>("select_directory");
  } catch (error) {
    throw asError(error);
  }
}

export async function openPath(path: string): Promise<void> {
  try {
    await invoke("open_path", { path });
  } catch (error) {
    throw asError(error);
  }
}

export async function getBinaryVersions(): Promise<BinaryVersions> {
  try {
    return await invoke<BinaryVersions>("get_binary_versions");
  } catch (error) {
    throw asError(error);
  }
}
