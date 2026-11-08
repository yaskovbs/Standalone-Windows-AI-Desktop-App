import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { HardwareInfo, ModelStatus, UpdateInfo } from "../types";

export interface SetupState {
  status: ModelStatus | null;
  hardware: HardwareInfo | null;
  online: boolean;
  update: UpdateInfo | null;
  progress: { downloaded: number; total: number } | null;
  busy: boolean;
  error: string | null;
  start: () => Promise<void>;
}

/** First-run flow: check connectivity, download the model if missing, load it into memory. */
export function useSetup(): SetupState {
  const [status, setStatus] = useState<ModelStatus | null>(null);
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [online, setOnline] = useState(false);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [progress, setProgress] = useState<SetupState["progress"]>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      let st = await api.modelStatus();
      if (!st.present) {
        await api.downloadModel((e) => {
          if (e.event === "progress") setProgress(e.data);
          if (e.event === "error") setError(e.data.message);
        });
        setProgress(null);
      }
      st = await api.loadModel();
      setStatus(st);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    (async () => {
      setHardware(await api.hardware());
      setStatus(await api.modelStatus());
      const isOnline = await api.isOnline();
      setOnline(isOnline);
      if (isOnline) api.checkUpdate().then(setUpdate).catch(() => undefined);
    })();
  }, []);

  // Auto-load when the model already exists on disk.
  useEffect(() => {
    if (status?.present && !status.loaded && !busy && !error) void start();
  }, [status, busy, error, start]);

  return { status, hardware, online, update, progress, busy, error, start };
}
