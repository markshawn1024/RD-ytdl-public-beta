import { useEffect } from "react";
import { useSettingsStore } from "../stores/settingsStore";

export function useAppConfig() {
  const settings = useSettingsStore((s) => s.settings);
  const load = useSettingsStore((s) => s.load);
  const save = useSettingsStore((s) => s.save);
  const versions = useSettingsStore((s) => s.versions);
  const saving = useSettingsStore((s) => s.saving);

  useEffect(() => {
    void load().catch(() => undefined);
  }, [load]);

  return { settings, versions, saving, load, save };
}
