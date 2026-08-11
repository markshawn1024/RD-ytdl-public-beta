import { create } from "zustand";
import type { AppConfig, BinaryVersions } from "../types/download";
import { getBinaryVersions, getSettings, updateSettings } from "../lib/tauri";

interface SettingsState {
  settings: AppConfig | null;
  versions: BinaryVersions | null;
  saving: boolean;
  load: () => Promise<void>;
  save: (settings: AppConfig) => Promise<AppConfig>;
  setLocal: (settings: AppConfig) => void;
}

const fallback: AppConfig = {
  downloadDir: "",
  concurrency: 2,
  proxy: "",
  downloadSubtitles: false,
  language: "zh-CN",
};

export const useSettingsStore = create<SettingsState>((set) => ({
  settings: null,
  versions: null,
  saving: false,
  load: async () => {
    const [settings, versions] = await Promise.all([
      getSettings(),
      getBinaryVersions().catch(() => null),
    ]);
    set({ settings, versions });
  },
  save: async (settings) => {
    set({ saving: true });
    try {
      const saved = await updateSettings(settings);
      set({ settings: saved });
      return saved;
    } finally {
      set({ saving: false });
    }
  },
  setLocal: (settings) => set({ settings }),
}));

export function useSettingsOrDefault(): AppConfig {
  return useSettingsStore((s) => s.settings) ?? fallback;
}
