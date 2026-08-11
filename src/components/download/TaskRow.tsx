import type { DownloadTask } from "../../types/download";
import { statusLabel } from "../../types/download";
import { Button } from "../common/Button";

interface Props {
  task: DownloadTask;
  onCancel?: (id: string) => void;
  onOpen?: (path: string) => void;
  showLogs?: boolean;
}

export function TaskRow({ task, onCancel, onOpen, showLogs }: Props) {
  const canCancel = ["queued", "running", "probing", "ready"].includes(task.status);
  const openTarget = task.outputPath || task.outputDir;

  return (
    <div className="rounded-2xl border border-[var(--line)] bg-[var(--card)] p-4 shadow-[var(--shadow)]">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded-full bg-white/70 px-2.5 py-0.5 text-xs text-[var(--muted)]">
              {statusLabel(task.status)}
            </span>
            {task.speed ? (
              <span className="text-xs text-[var(--muted)]">{task.speed}</span>
            ) : null}
            {task.eta ? (
              <span className="text-xs text-[var(--muted)]">ETA {task.eta}</span>
            ) : null}
          </div>
          <h3 className="mt-2 truncate text-lg leading-tight">
            {task.title || task.url}
          </h3>
          <p className="mt-1 truncate text-xs text-[var(--muted)]">{task.url}</p>
          {task.error ? (
            <p className="mt-2 text-sm text-[var(--danger)]">{task.error}</p>
          ) : null}
        </div>
        <div className="flex gap-2">
          {canCancel && onCancel ? (
            <Button variant="danger" onClick={() => onCancel(task.id)}>
              取消
            </Button>
          ) : null}
          {openTarget && onOpen ? (
            <Button variant="ghost" onClick={() => onOpen(openTarget)}>
              打开
            </Button>
          ) : null}
        </div>
      </div>

      <div className="mt-4 h-2 overflow-hidden rounded-full bg-[var(--bg-deep)]">
        <div
          className="h-full rounded-full bg-[var(--accent)] transition-all"
          style={{ width: `${Math.min(100, Math.max(0, task.progress))}%` }}
        />
      </div>
      <div className="mt-2 flex justify-between text-xs text-[var(--muted)]">
        <span>{task.progress.toFixed(1)}%</span>
        <span className="truncate pl-4">{task.outputPath || task.outputDir}</span>
      </div>

      {showLogs && task.logTail?.length ? (
        <pre className="mt-3 max-h-40 overflow-auto rounded-xl bg-[#1c1915] p-3 text-[11px] leading-relaxed text-[#f4efe6]">
          {task.logTail.slice(-20).join("\n")}
        </pre>
      ) : null}
    </div>
  );
}
