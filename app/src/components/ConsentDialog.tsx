import { useEffect, useState } from "react";
import { toolsApi } from "../lib/toolsApi";
import type { ShareStatus } from "../types";

/** One-time notice. Nothing is ever uploaded until the user clicks "Share". */
export default function ConsentDialog() {
  const [status, setStatus] = useState<ShareStatus | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    toolsApi.shareStatus().then(setStatus).catch(() => undefined);
  }, []);

  if (!status || !status.available || status.asked) return null;

  const choose = async (enabled: boolean) => {
    setBusy(true);
    try {
      setStatus(await toolsApi.setShareConsent(enabled));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4">
      <div className="max-h-full w-full max-w-lg overflow-y-auto rounded-2xl border border-line bg-panel p-6">
        <h2 className="text-lg font-semibold">Help the assistant learn?</h2>
        <p className="mt-2 text-sm text-slate-300">
          This app can improve over time by learning from its users. If you agree, <b>only answers you rate 👍</b> (your
          question and the reply) are sent to <code className="text-accent">{status.server}</code> and used to train future
          versions for everyone.
        </p>
        <ul className="mt-3 list-disc space-y-1 pl-5 text-sm text-slate-300">
          <li>Emails, phone numbers, keys, IP addresses and your Windows user name are removed automatically first. This is best-effort: please don't rate answers that contain private details.</li>
          <li>Never sent: videos, audio, screenshots, images, your settings, your GitHub token, or chats that include a video.</li>
          <li>You are identified only by a random ID, not by an account.</li>
          <li>You can stop anytime in Settings. "Withdraw" also deletes everything you shared.</li>
        </ul>
        <p className="mt-3 text-xs text-slate-500">If you say no, nothing is sent and the app works exactly the same.</p>
        <div className="mt-5 flex gap-3">
          <button disabled={busy} onClick={() => choose(true)} className="flex-1 rounded-lg border border-accent px-4 py-2 text-sm font-medium text-accent disabled:opacity-40">
            Share helpful answers
          </button>
          <button disabled={busy} onClick={() => choose(false)} className="flex-1 rounded-lg border border-line px-4 py-2 text-sm font-medium disabled:opacity-40">
            No thanks
          </button>
        </div>
      </div>
    </div>
  );
}
