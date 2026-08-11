import { useMemo } from "react";
import { TaskRow } from "../components/download/TaskRow";
import { Button } from "../components/common/Button";
import { useDownloadStore } from "../stores/downloadStore";
import { cancelDownload, clearFinished, openPath } from "../lib/tauri";

export function HistoryPage() {
  const tasks = useDownloadStore((s) => s.tasks);
  const upsertTask = useDownloadStore((s) => s.upsertTask);
  const refresh = useDownloadStore((s) => s.refresh);
  const removeFinishedLocally = useDownloadStore((s) => s.removeFinishedLocally);

  const history = useMemo(
    () =>
      tasks.filter((t) =>
        ["completed", "failed", "cancelled"].includes(t.status),
      ),
    [tasks],
  );

  return (
    <div className="space-y-5">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">
            History
          </p>
          <h2 className="mt-2 text-3xl">下载历史</h2>
          <p className="mt-2 text-sm text-[var(--muted)]">
            本地 JSON 持久化，最多保留近期记录。
          </p>
        </div>
        <Button
          variant="ghost"
          onClick={async () => {
            await clearFinished();
            removeFinishedLocally();
            await refresh();
          }}
        >
          清空已结束
        </Button>
      </div>

      {history.length === 0 ? (
        <div className="rounded-2xl border border-dashed border-[var(--line)] px-5 py-12 text-center text-sm text-[var(--muted)]">
          还没有历史记录
        </div>
      ) : (
        <div className="space-y-3">
          {history.map((task) => (
            <TaskRow
              key={task.id}
              task={task}
              onCancel={async (id) => {
                const updated = await cancelDownload(id);
                upsertTask(updated);
              }}
              onOpen={(path) => void openPath(path)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
