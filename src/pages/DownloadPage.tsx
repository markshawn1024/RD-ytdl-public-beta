import { useMemo, useState } from "react";
import { Button } from "../components/common/Button";
import { TaskRow } from "../components/download/TaskRow";
import { useDownloadStore } from "../stores/downloadStore";
import { useSettingsOrDefault } from "../stores/settingsStore";
import {
  cancelDownload,
  openPath,
  probeUrl,
  startDownload,
} from "../lib/tauri";
import type { AppError, VideoInfo } from "../types/download";
import { formatBytes, formatDuration } from "../types/download";

const PRESET = "bv*+ba/b";

export function DownloadPage() {
  const tasks = useDownloadStore((s) => s.tasks);
  const upsertTask = useDownloadStore((s) => s.upsertTask);
  const settings = useSettingsOrDefault();

  const [url, setUrl] = useState("");
  const [info, setInfo] = useState<VideoInfo | null>(null);
  const [formatId, setFormatId] = useState(PRESET);
  const [probing, setProbing] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeTasks = useMemo(
    () =>
      tasks.filter((t) =>
        ["queued", "running", "probing", "ready"].includes(t.status),
      ),
    [tasks],
  );

  const formatOptions = useMemo(() => {
    if (!info) return [];
    return info.formats.filter((f) => f.vcodec || f.acodec).slice(0, 40);
  }, [info]);

  async function onProbe() {
    setError(null);
    setInfo(null);
    setProbing(true);
    try {
      const result = await probeUrl(url.trim());
      setInfo(result);
      setFormatId(PRESET);
    } catch (e) {
      setError((e as AppError).message || "解析失败");
    } finally {
      setProbing(false);
    }
  }

  async function onStart() {
    setError(null);
    setStarting(true);
    try {
      const task = await startDownload({
        url: url.trim(),
        formatId,
        outputDir: settings.downloadDir || null,
        title: info?.title ?? null,
        thumbnail: info?.thumbnail ?? null,
      });
      upsertTask(task);
    } catch (e) {
      setError((e as AppError).message || "开始下载失败");
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="space-y-6">
      <section className="overflow-hidden rounded-3xl border border-[var(--line)] bg-[var(--card)] shadow-[var(--shadow)]">
        <div className="relative px-6 py-8 md:px-8">
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_80%_20%,rgba(15,107,92,0.12),transparent_45%)]" />
          <div className="relative">
            <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">
              Download
            </p>
            <h2 className="mt-2 max-w-xl text-3xl leading-tight md:text-4xl">
              粘贴链接，本地下载
            </h2>
            <p className="mt-3 max-w-2xl text-sm text-[var(--muted)]">
              基于内置 yt-dlp 与 ffmpeg，无需服务器。默认只下载单个视频。
            </p>

            <div className="mt-6 flex flex-col gap-3 md:flex-row">
              <input
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="https://www.youtube.com/watch?v=..."
                className="min-w-0 flex-1 rounded-xl border border-[var(--line)] bg-white/70 px-4 py-3 outline-none ring-[var(--accent)] focus:ring-2"
              />
              <Button onClick={onProbe} disabled={!url.trim() || probing}>
                {probing ? "解析中..." : "解析"}
              </Button>
            </div>
            {error ? (
              <p className="mt-3 text-sm text-[var(--danger)]">{error}</p>
            ) : null}
          </div>
        </div>
      </section>

      {info ? (
        <section className="rounded-3xl border border-[var(--line)] bg-[var(--card)] p-6 shadow-[var(--shadow)]">
          <div className="flex flex-col gap-5 md:flex-row">
            {info.thumbnail ? (
              <img
                src={info.thumbnail}
                alt=""
                className="h-36 w-full rounded-2xl object-cover md:h-40 md:w-64"
              />
            ) : (
              <div className="flex h-36 w-full items-center justify-center rounded-2xl bg-[var(--bg-deep)] text-sm text-[var(--muted)] md:h-40 md:w-64">
                无封面
              </div>
            )}
            <div className="min-w-0 flex-1">
              <h3 className="text-2xl leading-tight">{info.title}</h3>
              <p className="mt-2 text-sm text-[var(--muted)]">
                {info.uploader || "未知作者"} · 时长 {formatDuration(info.duration)}
              </p>

              <label className="mt-5 block text-sm text-[var(--muted)]">
                格式
              </label>
              <select
                value={formatId}
                onChange={(e) => setFormatId(e.target.value)}
                className="mt-2 w-full rounded-xl border border-[var(--line)] bg-white/70 px-3 py-2.5"
              >
                <option value={PRESET}>最佳视频+音频（自动合并）</option>
                {formatOptions.map((f) => (
                  <option key={f.formatId} value={f.formatId}>
                    {f.formatId}
                    {f.resolution ? ` · ${f.resolution}` : ""}
                    {f.ext ? ` · ${f.ext}` : ""}
                    {f.formatNote ? ` · ${f.formatNote}` : ""}
                    {` · ${formatBytes(f.filesize)}`}
                  </option>
                ))}
              </select>

              <div className="mt-5">
                <Button onClick={onStart} disabled={starting}>
                  {starting ? "加入队列..." : "开始下载"}
                </Button>
              </div>
            </div>
          </div>
        </section>
      ) : null}

      <section className="space-y-3">
        <div className="flex items-end justify-between">
          <div>
            <h3 className="text-xl">当前任务</h3>
            <p className="text-sm text-[var(--muted)]">
              并发上限 {settings.concurrency}
            </p>
          </div>
        </div>
        {activeTasks.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-[var(--line)] px-5 py-10 text-center text-sm text-[var(--muted)]">
            暂无进行中的下载
          </div>
        ) : (
          activeTasks.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              showLogs
              onCancel={async (id) => {
                const updated = await cancelDownload(id);
                upsertTask(updated);
              }}
              onOpen={(path) => void openPath(path)}
            />
          ))
        )}
      </section>
    </div>
  );
}
