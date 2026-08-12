import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import logoUrl from "./assets/logo.png";

type QueueTaskStatus =
  | "waiting"
  | "downloading"
  | "completed"
  | "failed"
  | "cancelled"
  | "interrupted";

type TabId = "queue" | "history" | "settings";

interface DownloadTask {
  id: string;
  url: string;
  title?: string | null;
  status: QueueTaskStatus;
  progress: number;
  speed?: string | null;
  eta?: string | null;
  createdAt: string;
  outputPath?: string | null;
  error?: string | null;
}

interface QueueSnapshot {
  paused: boolean;
  tasks: DownloadTask[];
  currentId?: string | null;
}

interface AppSettings {
  downloadDir: string;
  defaultFormat: string;
  defaultQuality: string;
  concurrency: number;
  proxy?: string | null;
  downloadSubtitles: boolean;
  language: string;
}

interface DownloadLog {
  status: string;
  message: string;
  percent?: number | null;
  speed?: string | null;
  eta?: string | null;
}

interface SidecarVersions {
  ytDlp: string;
  ffmpeg: string;
}

interface VideoFormatInfo {
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

interface VideoInfoResult {
  title: string;
  thumbnail?: string | null;
  duration?: number | null;
  uploader?: string | null;
  formats: VideoFormatInfo[];
  webpageUrl?: string | null;
  id?: string | null;
}

interface AppError {
  code?: string;
  message: string;
}

function formatDuration(seconds?: number | null): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "未知";
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) {
    return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }
  return `${m}:${String(s).padStart(2, "0")}`;
}

function formatLogLine(log: DownloadLog): string {
  if (log.status === "downloading") {
    const bits = ["下载中:"];
    if (log.percent != null) bits.push(`${log.percent.toFixed(1)}%`);
    if (log.speed) bits.push(log.speed);
    if (log.eta) bits.push(`剩余 ${log.eta}`);
    if (!log.percent && log.message) bits.push(log.message);
    return bits.join(" ");
  }
  if (log.status === "merging") return `合并中: ${log.message || "正在合并音视频"}`;
  if (log.status === "completed") return log.message || "下载完成";
  if (log.status === "failed") return `失败: ${log.message}`;
  if (log.status === "cancelled") return `已取消: ${log.message || "任务已取消"}`;
  if (log.status === "checking") return log.message || "正在检查环境...";
  return log.message;
}

function statusMeta(status: QueueTaskStatus): { label: string; className: string } {
  switch (status) {
    case "waiting":
      return { label: "等待中", className: "rd-badge-waiting" };
    case "downloading":
      return { label: "下载中", className: "rd-badge-downloading" };
    case "completed":
      return { label: "已完成", className: "rd-badge-completed" };
    case "failed":
      return { label: "失败", className: "rd-badge-failed" };
    case "cancelled":
      return { label: "已取消", className: "rd-badge-cancelled" };
    case "interrupted":
      return { label: "已中断", className: "rd-badge-interrupted" };
  }
}

function StatusBadge({ status }: { status: QueueTaskStatus }) {
  const meta = statusMeta(status);
  return <span className={`rd-badge ${meta.className}`}>{meta.label}</span>;
}

function TaskCard({
  task,
  onRemove,
}: {
  task: DownloadTask;
  onRemove?: (id: string) => void;
}) {
  return (
    <li className="rd-card rounded-[14px] border border-[var(--rd-border)] px-3.5 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="mb-1.5 flex flex-wrap items-center gap-2">
            <StatusBadge status={task.status} />
            {task.status === "downloading" && task.speed ? (
              <span className="text-[11px] text-[var(--rd-metal)]">{task.speed}</span>
            ) : null}
            {task.status === "downloading" && task.eta ? (
              <span className="text-[11px] text-[var(--rd-metal)]">剩余 {task.eta}</span>
            ) : null}
          </div>
          <div className="truncate text-sm font-medium text-[var(--rd-ink)]">
            {task.title || task.url}
          </div>
          <div className="mt-0.5 truncate text-xs text-[var(--rd-muted)]">{task.url}</div>
          {task.error ? (
            <div className="mt-1.5 text-xs text-[#ff8a8a]">{task.error}</div>
          ) : null}
          {task.outputPath ? (
            <div className="mt-1.5 truncate text-xs text-[var(--rd-metal)]">
              {task.outputPath}
            </div>
          ) : null}
        </div>
        {onRemove ? (
          <button
            type="button"
            className="shrink-0 text-xs text-[#ff8a8a] hover:underline"
            onClick={() => onRemove(task.id)}
          >
            删除
          </button>
        ) : null}
      </div>
      {task.status === "downloading" ? (
        <div className="mt-3">
          <div className="mb-1 flex justify-between text-[11px] text-[var(--rd-metal)]">
            <span>进度</span>
            <span>{task.progress.toFixed(0)}%</span>
          </div>
          <div className="rd-progress">
            <span style={{ width: `${Math.min(100, Math.max(0, task.progress))}%` }} />
          </div>
        </div>
      ) : null}
    </li>
  );
}

export default function App() {
  const [url, setUrl] = useState("");
  const [tab, setTab] = useState<TabId>("queue");
  const [queue, setQueue] = useState<QueueSnapshot>({
    paused: false,
    tasks: [],
    currentId: null,
  });
  const [logs, setLogs] = useState<string[]>([]);
  const [versions, setVersions] = useState<SidecarVersions | null>(null);
  const [versionError, setVersionError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [parsing, setParsing] = useState(false);
  const [videoInfo, setVideoInfo] = useState<VideoInfoResult | null>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [settingsMsg, setSettingsMsg] = useState<string | null>(null);
  const [liveStatus, setLiveStatus] = useState<string | null>(null);
  const consoleRef = useRef<HTMLPreElement>(null);

  const current = useMemo(
    () => queue.tasks.filter((t) => t.status === "downloading"),
    [queue.tasks],
  );
  const waiting = useMemo(
    () => queue.tasks.filter((t) => t.status === "waiting"),
    [queue.tasks],
  );
  const history = useMemo(
    () =>
      queue.tasks.filter((t) =>
        ["completed", "failed", "cancelled", "interrupted"].includes(t.status),
      ),
    [queue.tasks],
  );
  const completed = useMemo(
    () => history.filter((t) => t.status === "completed"),
    [history],
  );
  const failed = useMemo(
    () => history.filter((t) => t.status === "failed"),
    [history],
  );
  const cancelled = useMemo(
    () => history.filter((t) => t.status === "cancelled"),
    [history],
  );
  const interrupted = useMemo(
    () => history.filter((t) => t.status === "interrupted"),
    [history],
  );

  const hasCurrent = current.length > 0;
  const currentTask = current[0];

  useEffect(() => {
    const unsubs: Array<() => void> = [];

    void (async () => {
      try {
        const v = await invoke<SidecarVersions>("check_sidecars");
        setVersions(v);
        setVersionError(null);
      } catch (e) {
        setVersionError((e as AppError).message || String(e));
      }

      try {
        const snap = await invoke<QueueSnapshot>("queue_list");
        setQueue(snap);
        if (snap.paused && snap.tasks.some((t) => t.status === "waiting")) {
          setLogs((p) => [
            ...p,
            "已从本地恢复等待任务；队列已暂停。点击「继续队列」开始下载。",
          ]);
        }
      } catch {
        /* empty */
      }

      try {
        const s = await invoke<AppSettings>("get_settings");
        setSettings(s);
      } catch {
        /* empty */
      }

      unsubs.push(
        await listen<QueueSnapshot>("queue://updated", (event) => {
          setQueue(event.payload);
        }),
      );
      unsubs.push(
        await listen<DownloadLog>("download://log", (event) => {
          setLiveStatus(event.payload.status);
          setLogs((prev) => [...prev, formatLogLine(event.payload)]);
        }),
      );
      unsubs.push(
        await listen<AppSettings>("settings://changed", (event) => {
          setSettings(event.payload);
        }),
      );
    })();

    return () => unsubs.forEach((fn) => fn());
  }, []);

  useEffect(() => {
    if (consoleRef.current) {
      consoleRef.current.scrollTop = consoleRef.current.scrollHeight;
    }
  }, [logs]);

  async function onParse() {
    const trimmed = url.trim();
    if (!trimmed) {
      setParseError("请输入视频链接");
      setLogs((p) => [...p, "失败: 请输入视频链接"]);
      return;
    }
    setParsing(true);
    setParseError(null);
    setVideoInfo(null);
    try {
      const info = await invoke<VideoInfoResult>("get_video_info", { url: trimmed });
      setVideoInfo(info);
      setLogs((p) => [...p, `解析成功: ${info.title}`]);
    } catch (e) {
      const msg = (e as AppError).message || String(e);
      setParseError(msg);
      setLogs((p) => [...p, `解析失败: ${msg}`]);
    } finally {
      setParsing(false);
    }
  }

  async function onConfirmAdd() {
    const trimmed = url.trim();
    if (!trimmed || !videoInfo) {
      setLogs((p) => [...p, "失败: 请先解析视频"]);
      return;
    }
    setAdding(true);
    try {
      await invoke("queue_add", { url: trimmed });
      setLogs((p) => [...p, `已加入队列: ${videoInfo.title}`]);
      setUrl("");
      setVideoInfo(null);
      setParseError(null);
      setTab("queue");
    } catch (e) {
      setLogs((p) => [...p, `失败: ${(e as AppError).message || String(e)}`]);
    } finally {
      setAdding(false);
    }
  }

  function onClearPreview() {
    setVideoInfo(null);
    setParseError(null);
  }

  async function onSaveSettings() {
    if (!settings) return;
    setSettingsMsg(null);
    try {
      const saved = await invoke<AppSettings>("update_settings", { settings });
      setSettings(saved);
      setSettingsMsg("设置已保存");
    } catch (e) {
      setSettingsMsg((e as AppError).message || String(e));
    }
  }

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-5xl flex-col gap-4 px-4 py-5 sm:px-6 sm:py-6">
      {/* Header */}
      <header className="rd-card rd-card-accent flex items-center gap-3 px-4 py-3.5 sm:px-5">
        <img
          src={logoUrl}
          alt="Racing Dispatch"
          className="h-12 w-12 shrink-0 rounded-xl object-cover sm:h-14 sm:w-14"
        />
        <div className="min-w-0">
          <div className="text-[11px] font-semibold tracking-[0.28em] text-[var(--rd-red)]">
            RACING DISPATCH
          </div>
          <h1 className="truncate text-lg font-semibold tracking-tight sm:text-xl">
            Media Toolkit
          </h1>
          <p className="truncate text-xs text-[var(--rd-muted)]">
            专业赛车媒体本地下载工具
          </p>
        </div>
        <div className="ml-auto hidden text-right text-[11px] text-[var(--rd-metal)] sm:block">
          <div>队列 {queue.paused ? "已暂停" : "运行中"}</div>
          {versions ? (
            <div className="mt-0.5 font-mono opacity-80">yt-dlp {versions.ytDlp}</div>
          ) : null}
        </div>
      </header>

      {versionError ? (
        <div className="rd-card border-[var(--rd-border-red)] px-4 py-3 text-sm text-[#ff8a8a]">
          {versionError}
        </div>
      ) : null}

      {/* URL + parse preview */}
      <section className="rd-card space-y-3 px-4 py-4 sm:px-5">
        <label className="text-xs font-medium tracking-wide text-[var(--rd-metal)]" htmlFor="video-url">
          视频链接
        </label>
        <input
          id="video-url"
          value={url}
          onChange={(e) => {
            setUrl(e.target.value);
            if (videoInfo || parseError) {
              setVideoInfo(null);
              setParseError(null);
            }
          }}
          placeholder="粘贴视频 URL，例如 https://..."
          className="rd-input"
          onKeyDown={(e) => {
            if (e.key === "Enter" && !parsing && !adding) void onParse();
          }}
        />
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void onParse()}
            disabled={parsing || !!versionError}
            className="rd-btn rd-btn-primary"
          >
            {parsing ? "解析中..." : "解析视频"}
          </button>
          <button
            type="button"
            onClick={() => void onConfirmAdd()}
            disabled={adding || !videoInfo || !!versionError}
            className="rd-btn rd-btn-primary"
          >
            {adding ? "加入中..." : "加入队列"}
          </button>
          <button
            type="button"
            disabled={!hasCurrent}
            onClick={() => void invoke("queue_cancel_current")}
            className="rd-btn rd-btn-danger"
          >
            取消任务
          </button>
          {queue.paused ? (
            <button
              type="button"
              onClick={() => void invoke("queue_resume")}
              className="rd-btn rd-btn-ghost"
            >
              继续队列
            </button>
          ) : (
            <button
              type="button"
              onClick={() => void invoke("queue_pause")}
              className="rd-btn rd-btn-ghost"
            >
              暂停等待
            </button>
          )}
          <button
            type="button"
            onClick={() =>
              void invoke<string>("open_download_folder").then((path) =>
                setLogs((p) => [...p, `已打开文件夹: ${path}`]),
              )
            }
            className="rd-btn rd-btn-ghost"
          >
            打开文件夹
          </button>
        </div>

        {parseError ? (
          <div className="rounded-xl border border-[var(--rd-border-red)] bg-[rgba(220,38,38,0.08)] px-3.5 py-3 text-sm text-[#ff8a8a]">
            {parseError}
          </div>
        ) : null}

        {videoInfo ? (
          <div className="rd-card overflow-hidden rounded-[14px] border border-[var(--rd-border)] !bg-[rgba(0,0,0,0.25)]">
            <div className="flex flex-col gap-3 p-3 sm:flex-row sm:items-start sm:gap-4 sm:p-4">
              <div className="relative aspect-video w-full shrink-0 overflow-hidden rounded-lg bg-black/40 sm:w-44">
                {videoInfo.thumbnail ? (
                  <img
                    src={videoInfo.thumbnail}
                    alt={videoInfo.title}
                    className="h-full w-full object-cover"
                    referrerPolicy="no-referrer"
                  />
                ) : (
                  <div className="flex h-full items-center justify-center text-xs text-[var(--rd-muted)]">
                    无缩略图
                  </div>
                )}
              </div>
              <div className="min-w-0 flex-1 space-y-2">
                <div>
                  <div className="text-[11px] font-medium tracking-wide text-[var(--rd-metal)]">
                    视频标题
                  </div>
                  <h2 className="mt-0.5 text-sm font-semibold leading-snug text-[var(--rd-ink)] sm:text-base">
                    {videoInfo.title}
                  </h2>
                </div>
                <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-3">
                  <div>
                    <div className="text-[var(--rd-metal)]">来源</div>
                    <div className="mt-0.5 truncate text-[var(--rd-ink)]">
                      {videoInfo.uploader || "未知"}
                    </div>
                  </div>
                  <div>
                    <div className="text-[var(--rd-metal)]">时长</div>
                    <div className="mt-0.5 text-[var(--rd-ink)]">
                      {formatDuration(videoInfo.duration)}
                    </div>
                  </div>
                  <div className="col-span-2 sm:col-span-1">
                    <div className="text-[var(--rd-metal)]">可用格式</div>
                    <div className="mt-0.5 text-[var(--rd-ink)]">
                      {videoInfo.formats.length} 种
                    </div>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2 pt-1">
                  <button
                    type="button"
                    onClick={() => void onConfirmAdd()}
                    disabled={adding || !!versionError}
                    className="rd-btn rd-btn-primary"
                  >
                    {adding ? "加入中..." : "确认并加入队列"}
                  </button>
                  <button type="button" onClick={onClearPreview} className="rd-btn rd-btn-ghost">
                    清除预览
                  </button>
                </div>
              </div>
            </div>
          </div>
        ) : null}
      </section>

      {/* Current task */}
      <section className="rd-card rd-card-accent px-4 py-4 sm:px-5">
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-sm font-semibold tracking-wide">当前任务</h2>
          {liveStatus === "merging" ? (
            <span className="rd-badge rd-badge-merging">合并中</span>
          ) : null}
        </div>
        {currentTask ? (
          <div>
            <div className="mb-2 flex flex-wrap items-center gap-2">
              <StatusBadge status="downloading" />
              <span className="truncate text-sm">{currentTask.title || currentTask.url}</span>
            </div>
            <div className="mb-2 flex justify-between text-xs text-[var(--rd-metal)]">
              <span>
                {currentTask.speed || "—"}
                {currentTask.eta ? ` · 剩余 ${currentTask.eta}` : ""}
              </span>
              <span className="font-semibold text-[var(--rd-red)]">
                {currentTask.progress.toFixed(0)}%
              </span>
            </div>
            <div className="rd-progress">
              <span
                style={{
                  width: `${Math.min(100, Math.max(0, currentTask.progress))}%`,
                }}
              />
            </div>
          </div>
        ) : (
          <p className="text-sm text-[var(--rd-muted)]">暂无进行中的下载</p>
        )}
      </section>

      {/* Tabs */}
      <section className="rd-card overflow-hidden">
        <div className="flex flex-wrap gap-1 border-b border-[var(--rd-border)] px-3 py-2">
          {(
            [
              { id: "queue" as const, label: "下载队列" },
              { id: "history" as const, label: "下载历史" },
              { id: "settings" as const, label: "设置" },
            ] as const
          ).map((item) => (
            <button
              key={item.id}
              type="button"
              className={`rd-tab ${tab === item.id ? "rd-tab-active" : ""}`}
              onClick={() => setTab(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="space-y-4 px-4 py-4 sm:px-5">
          {tab === "queue" ? (
            <>
              <div className="space-y-2">
                <h3 className="text-xs uppercase tracking-[0.16em] text-[var(--rd-muted)]">
                  等待中（{waiting.length}）
                </h3>
                {waiting.length === 0 ? (
                  <p className="text-sm text-[var(--rd-muted)]">队列为空</p>
                ) : (
                  <ul className="space-y-2">
                    {waiting.map((task) => (
                      <TaskCard
                        key={task.id}
                        task={task}
                        onRemove={(id) => {
                          void invoke("queue_remove_waiting", { id }).catch(
                            (e: AppError) =>
                              setLogs((p) => [
                                ...p,
                                `失败: ${e.message || String(e)}`,
                              ]),
                          );
                        }}
                      />
                    ))}
                  </ul>
                )}
              </div>
            </>
          ) : null}

          {tab === "history" ? (
            <>
              {[
                { title: "已完成", tasks: completed },
                { title: "失败", tasks: failed },
                { title: "已取消", tasks: cancelled },
                { title: "已中断", tasks: interrupted },
              ].map((group) =>
                group.tasks.length ? (
                  <div key={group.title} className="space-y-2">
                    <h3 className="text-xs uppercase tracking-[0.16em] text-[var(--rd-muted)]">
                      {group.title}（{group.tasks.length}）
                    </h3>
                    <ul className="space-y-2">
                      {group.tasks.map((task) => (
                        <TaskCard key={task.id} task={task} />
                      ))}
                    </ul>
                  </div>
                ) : null,
              )}
              {history.length === 0 ? (
                <p className="text-sm text-[var(--rd-muted)]">暂无历史记录</p>
              ) : null}
            </>
          ) : null}

          {tab === "settings" && settings ? (
            <div className="space-y-3">
              <label className="block text-xs text-[var(--rd-metal)]">下载目录</label>
              <div className="flex flex-col gap-2 sm:flex-row">
                <input
                  value={settings.downloadDir}
                  onChange={(e) =>
                    setSettings({ ...settings, downloadDir: e.target.value })
                  }
                  className="rd-input"
                />
                <button
                  type="button"
                  className="rd-btn rd-btn-ghost shrink-0"
                  onClick={() =>
                    void invoke<string | null>("select_directory").then((dir) => {
                      if (dir) setSettings({ ...settings, downloadDir: dir });
                    })
                  }
                >
                  选择目录
                </button>
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <label className="text-xs text-[var(--rd-metal)]">默认格式</label>
                  <input
                    value={settings.defaultFormat}
                    onChange={(e) =>
                      setSettings({ ...settings, defaultFormat: e.target.value })
                    }
                    className="rd-input mt-1 font-mono text-sm"
                  />
                </div>
                <div>
                  <label className="text-xs text-[var(--rd-metal)]">默认质量</label>
                  <select
                    value={settings.defaultQuality}
                    onChange={(e) =>
                      setSettings({ ...settings, defaultQuality: e.target.value })
                    }
                    className="rd-input mt-1"
                  >
                    <option value="best">最佳</option>
                    <option value="1080p">1080p</option>
                    <option value="720p">720p</option>
                    <option value="480p">480p</option>
                  </select>
                </div>
              </div>
              <button
                type="button"
                onClick={() => void onSaveSettings()}
                className="rd-btn rd-btn-primary"
              >
                保存设置
              </button>
              {settingsMsg ? (
                <p className="text-sm text-[var(--rd-metal)]">{settingsMsg}</p>
              ) : null}
              {versions ? (
                <dl className="grid gap-1 border-t border-[var(--rd-border)] pt-3 text-xs text-[var(--rd-muted)]">
                  <div>
                    yt-dlp：
                    <span className="ml-1 font-mono text-[var(--rd-metal-bright)]">
                      {versions.ytDlp}
                    </span>
                  </div>
                  <div>
                    ffmpeg：
                    <span className="ml-1 font-mono text-[var(--rd-metal-bright)]">
                      {versions.ffmpeg}
                    </span>
                  </div>
                </dl>
              ) : null}
            </div>
          ) : null}
        </div>
      </section>

      {/* Log */}
      <section className="rd-card flex min-h-0 flex-col gap-2 px-4 py-4 sm:px-5">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold">运行日志</h2>
          <button
            type="button"
            className="text-xs text-[var(--rd-muted)] hover:text-[var(--rd-ink)]"
            onClick={() => setLogs([])}
          >
            清空
          </button>
        </div>
        <pre
          ref={consoleRef}
          className="h-[200px] overflow-auto rounded-xl border border-[var(--rd-border)] bg-black/60 p-3 font-mono text-[11px] leading-relaxed text-[#d4d4dc] sm:h-[240px]"
        >
          {logs.length === 0 ? "等待任务..." : logs.join("\n")}
        </pre>
      </section>
    </div>
  );
}
