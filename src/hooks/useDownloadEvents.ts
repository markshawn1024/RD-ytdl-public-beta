import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig, DownloadTask, LogEvent, ProgressEvent } from "../types/download";
import { useDownloadStore } from "../stores/downloadStore";
import { useSettingsStore } from "../stores/settingsStore";

export function useDownloadEvents() {
  const upsertTask = useDownloadStore((s) => s.upsertTask);
  const applyProgress = useDownloadStore((s) => s.applyProgress);
  const appendLog = useDownloadStore((s) => s.appendLog);
  const refresh = useDownloadStore((s) => s.refresh);
  const setLocal = useSettingsStore((s) => s.setLocal);

  useEffect(() => {
    void refresh();

    const unsubs = Promise.all([
      listen<DownloadTask>("download://task-upsert", (event) => {
        upsertTask(event.payload);
      }),
      listen<ProgressEvent>("download://progress", (event) => {
        applyProgress(event.payload);
      }),
      listen<LogEvent>("download://log", (event) => {
        appendLog(event.payload.id, event.payload.line);
      }),
      listen<AppConfig>("settings://changed", (event) => {
        setLocal(event.payload);
      }),
    ]);

    return () => {
      void unsubs.then((fns) => fns.forEach((fn) => fn()));
    };
  }, [appendLog, applyProgress, refresh, setLocal, upsertTask]);
}
