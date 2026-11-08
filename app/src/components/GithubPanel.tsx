import { useEffect, useState } from "react";
import { toolsApi } from "../lib/toolsApi";
import type { FileChange, GithubStatus, Proposal } from "../types";
import Modal from "./Modal";

function DiffBlock({ change }: { change: FileChange }) {
  const { diff } = change;
  return (
    <div className="mb-3 overflow-hidden rounded-lg border border-line">
      <div className="bg-bg px-3 py-1 text-xs text-slate-300">
        {change.path} {change.is_new && <span className="text-emerald-300">(new file)</span>}
      </div>
      <pre className="max-h-64 overflow-auto p-2 text-xs leading-5">
        {diff.before.map((l, i) => <div key={`b${i}`} className="text-slate-500">  {l}</div>)}
        {diff.removed.map((l, i) => <div key={`r${i}`} className="bg-red-500/15 text-red-300">- {l}</div>)}
        {diff.added.map((l, i) => <div key={`a${i}`} className="bg-emerald-500/15 text-emerald-300">+ {l}</div>)}
        {diff.after.map((l, i) => <div key={`f${i}`} className="text-slate-500">  {l}</div>)}
      </pre>
    </div>
  );
}

export default function GithubPanel({ onClose }: { onClose: () => void }) {
  const [status, setStatus] = useState<GithubStatus | null>(null);
  const [token, setToken] = useState("");
  const [instruction, setInstruction] = useState("");
  const [paths, setPaths] = useState("app/src/components/Sidebar.tsx");
  const [proposal, setProposal] = useState<Proposal | null>(null);
  const [busy, setBusy] = useState(false);
  const [prUrl, setPrUrl] = useState("");
  const [message, setMessage] = useState("");
  const [error, setError] = useState<string | null>(null);

  const refresh = () => toolsApi.githubStatus().then(setStatus);
  useEffect(() => {
    refresh();
  }, []);

  const run = async (fn: () => Promise<void>) => {
    setError(null);
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const saveToken = () =>
    run(async () => {
      setMessage(await toolsApi.githubSaveToken(token));
      setToken("");
      await refresh();
    });

  const propose = () =>
    run(async () => {
      setPrUrl("");
      setProposal(null);
      const list = paths.split("\n").map((p) => p.trim()).filter(Boolean);
      setProposal(await toolsApi.githubPropose(instruction, list));
    });

  const approve = () =>
    run(async () => {
      if (!proposal) return;
      setPrUrl(await toolsApi.githubApprove(proposal.id));
      setProposal(null);
    });

  const reject = async () => {
    if (proposal) await toolsApi.githubReject(proposal.id);
    setProposal(null);
  };

  const field = "w-full rounded-lg border border-line bg-bg px-3 py-2 text-sm outline-none focus:border-accent";

  return (
    <Modal title="Self-update via GitHub" onClose={onClose}>
      <p className="mb-4 text-xs text-slate-400">
        The assistant drafts a change; you review the diff; only then is a <b>draft pull request</b> opened on an{" "}
        <code>ai/…</code> branch of <b>{status?.repo ?? "..."}</b>. It never pushes to the main branch and never merges.
        It can only touch <code>app/src</code>, <code>app/src-tauri/src</code> and <code>pipeline</code>, and it cannot edit
        its own safety code.
      </p>

      {!status?.has_token ? (
        <div className="mb-4">
          <label className="block text-sm">
            GitHub fine-grained token (only this repo; Contents + Pull requests: read/write)
            <input className={field} type="password" value={token} onChange={(e) => setToken(e.target.value)} />
          </label>
          <button
            className="mt-2 rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black disabled:opacity-40"
            disabled={!token.trim() || busy}
            onClick={saveToken}
          >
            Save token (stored in Windows Credential Manager)
          </button>
        </div>
      ) : (
        <div className="mb-4 flex items-center justify-between text-sm text-emerald-300">
          <span>✓ Token saved {message && `- ${message}`}</span>
          <button className="text-xs text-slate-400 hover:text-red-300" onClick={() => toolsApi.githubClearToken().then(refresh)}>
            Remove token
          </button>
        </div>
      )}

      {status?.has_token && !proposal && (
        <div className="space-y-2">
          <label className="block text-sm">What should change?
            <textarea className={field} rows={3} value={instruction} onChange={(e) => setInstruction(e.target.value)} />
          </label>
          <label className="block text-sm">Files it may edit (one per line)
            <textarea className={field} rows={3} value={paths} onChange={(e) => setPaths(e.target.value)} />
          </label>
          <button
            className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black disabled:opacity-40"
            disabled={busy || !instruction.trim()}
            onClick={propose}
          >
            {busy ? "Drafting (this can take a minute)..." : "Draft a change"}
          </button>
        </div>
      )}

      {proposal && (
        <div>
          <div className="mb-2 text-sm font-medium">{proposal.title}</div>
          {proposal.changes.map((c) => <DiffBlock key={c.path} change={c} />)}
          <p className="mb-2 text-xs text-amber-300">
            Small local models make mistakes. Read every line before approving - nothing has been compiled or tested.
          </p>
          <div className="flex gap-2">
            <button className="rounded-lg bg-accent px-4 py-2 text-sm font-medium text-black disabled:opacity-40" disabled={busy} onClick={approve}>
              {busy ? "Opening PR..." : "Approve → open draft PR"}
            </button>
            <button className="rounded-lg border border-line px-4 py-2 text-sm" disabled={busy} onClick={reject}>
              Reject
            </button>
          </div>
        </div>
      )}

      {prUrl && (
        <p className="mt-4 text-sm">
          Draft PR opened:{" "}
          <a className="text-accent underline" href={prUrl} target="_blank" rel="noreferrer">{prUrl}</a>
        </p>
      )}
      {error && <p className="mt-4 rounded-lg bg-red-500/10 p-2 text-sm text-red-300">{error}</p>}
    </Modal>
  );
}
