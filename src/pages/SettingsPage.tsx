import { useEffect, useState } from "react";
import { Button } from "../components/common/Button";
import { useAppConfig } from "../hooks/useAppConfig";
import { selectDirectory } from "../lib/tauri";
import type { AppConfig } from "../types/download";

export function SettingsPage() {
  const { settings, versions, saving, save, load } = useAppConfig();
  const [draft, setDraft] = useState<AppConfig | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (settings) setDraft(settings);
  }, [settings]);

  if (!draft) {
    return (
      <div className="rounded-2xl border border-[var(--line)] bg-[var(--card)] p-8 text-sm text-[var(--muted)]">
        正在加载设置...
      </div>
    );
  }

  return (
    <div className="space-y-5">
      <div>
        <p className="text-xs uppercase tracking-[0.22em] text-[var(--muted)]">
          Settings
        </p>
        <h2 className="mt-2 text-3xl">设置</h2>
        <p className="mt-2 text-sm text-[var(--muted)]">
          下载目录、并发与代理均保存在本地。
        </p>
      </div>

      <section className="space-y-5 rounded-3xl border border-[var(--line)] bg-[var(--card)] p-6 shadow-[var(--shadow)]">
        <div>
          <label className="text-sm text-[var(--muted)]">默认下载目录</label>
          <div className="mt-2 flex flex-col gap-2 md:flex-row">
            <input
              value={draft.downloadDir}
              onChange={(e) =>
                setDraft({ ...draft, downloadDir: e.target.value })
              }
              className="min-w-0 flex-1 rounded-xl border border-[var(--line)] bg-white/70 px-3 py-2.5"
            />
            <Button
              variant="ghost"
              onClick={async () => {
                const dir = await selectDirectory();
                if (dir) setDraft({ ...draft, downloadDir: dir });
              }}
            >
              选择目录
            </Button>
          </div>
        </div>

        <div>
          <label className="text-sm text-[var(--muted)]">
            并发下载数（1–4）
          </label>
          <input
            type="number"
            min={1}
            max={4}
            value={draft.concurrency}
            onChange={(e) =>
              setDraft({
                ...draft,
                concurrency: Number(e.target.value) || 1,
              })
            }
            className="mt-2 w-32 rounded-xl border border-[var(--line)] bg-white/70 px-3 py-2.5"
          />
        </div>

        <div>
          <label className="text-sm text-[var(--muted)]">
            HTTP(S) 代理（可选）
          </label>
          <input
            value={draft.proxy ?? ""}
            onChange={(e) =>
              setDraft({ ...draft, proxy: e.target.value || null })
            }
            placeholder="http://127.0.0.1:7890"
            className="mt-2 w-full rounded-xl border border-[var(--line)] bg-white/70 px-3 py-2.5"
          />
        </div>

        <label className="flex items-center gap-3 text-sm">
          <input
            type="checkbox"
            checked={draft.downloadSubtitles}
            onChange={(e) =>
              setDraft({ ...draft, downloadSubtitles: e.target.checked })
            }
          />
          尝试下载字幕（zh / en）
        </label>

        <div className="flex flex-wrap gap-2">
          <Button
            disabled={saving}
            onClick={async () => {
              setMessage(null);
              setError(null);
              try {
                await save(draft);
                setMessage("已保存");
              } catch (e) {
                setError(e instanceof Error ? e.message : String(e));
              }
            }}
          >
            {saving ? "保存中..." : "保存设置"}
          </Button>
          <Button variant="ghost" onClick={() => void load()}>
            重新加载
          </Button>
        </div>

        {message ? <p className="text-sm text-[var(--accent)]">{message}</p> : null}
        {error ? <p className="text-sm text-[var(--danger)]">{error}</p> : null}
      </section>

      <section className="rounded-3xl border border-[var(--line)] bg-[var(--card)] p-6 shadow-[var(--shadow)]">
        <h3 className="text-lg">内置二进制</h3>
        <p className="mt-2 text-sm text-[var(--muted)]">
          若版本为空，请先运行 <code>npm run fetch-binaries</code>
        </p>
        <dl className="mt-4 space-y-2 text-sm">
          <div className="flex justify-between gap-4 border-b border-[var(--line)] py-2">
            <dt className="text-[var(--muted)]">yt-dlp</dt>
            <dd className="font-mono text-right">{versions?.ytDlp || "未检测到"}</dd>
          </div>
          <div className="flex justify-between gap-4 border-b border-[var(--line)] py-2">
            <dt className="text-[var(--muted)]">deno</dt>
            <dd className="font-mono text-right">{versions?.deno || "未检测到"}</dd>
          </div>
          <div className="flex justify-between gap-4 py-2">
            <dt className="text-[var(--muted)]">ffmpeg</dt>
            <dd className="font-mono text-right">{versions?.ffmpeg || "未检测到"}</dd>
          </div>
        </dl>
      </section>
    </div>
  );
}
