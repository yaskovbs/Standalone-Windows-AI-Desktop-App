import type { ReactNode } from "react";

export default function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-black/60 p-4">
      <div className="max-h-full w-full max-w-2xl overflow-y-auto rounded-2xl border border-line bg-panel p-5">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-lg font-semibold">{title}</h2>
          <button aria-label="Close" onClick={onClose} className="text-slate-400 hover:text-slate-100">
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
