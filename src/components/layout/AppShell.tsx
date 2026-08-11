import type { PropsWithChildren } from "react";

type Page = "download" | "history" | "settings";

interface Props {
  page: Page;
  onChange: (page: Page) => void;
}

const items: { id: Page; label: string; hint: string }[] = [
  { id: "download", label: "下载", hint: "解析与抓取" },
  { id: "history", label: "历史", hint: "完成与失败" },
  { id: "settings", label: "设置", hint: "目录与并发" },
];

export function AppShell({
  page,
  onChange,
  children,
}: PropsWithChildren<Props>) {
  return (
    <div className="min-h-screen px-5 py-5 md:px-8 md:py-6">
      <div className="mx-auto flex min-h-[calc(100vh-2.5rem)] max-w-6xl flex-col gap-6 md:flex-row">
        <aside className="md:w-56 md:shrink-0">
          <div className="sticky top-5 rounded-2xl border border-[var(--line)] bg-[var(--card)] p-5 shadow-[var(--shadow)] backdrop-blur">
            <div className="mb-8">
              <p className="text-[11px] uppercase tracking-[0.24em] text-[var(--muted)]">
                Local Toolkit
              </p>
              <h1 className="mt-2 font-serif text-3xl leading-none tracking-tight">
                RD Media
              </h1>
              <p className="mt-2 text-sm text-[var(--muted)]">视频下载 · 第一阶段</p>
            </div>
            <nav className="flex gap-2 md:flex-col">
              {items.map((item) => {
                const active = page === item.id;
                return (
                  <button
                    key={item.id}
                    onClick={() => onChange(item.id)}
                    className={`rounded-xl px-3 py-3 text-left transition ${
                      active
                        ? "bg-[var(--accent-soft)] text-[var(--accent)]"
                        : "hover:bg-white/50 text-[var(--ink)]"
                    }`}
                  >
                    <div className="text-sm font-medium">{item.label}</div>
                    <div className="text-xs text-[var(--muted)]">{item.hint}</div>
                  </button>
                );
              })}
            </nav>
          </div>
        </aside>
        <main className="min-w-0 flex-1">{children}</main>
      </div>
    </div>
  );
}
