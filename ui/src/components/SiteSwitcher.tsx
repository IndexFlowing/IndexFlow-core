"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { ChevronsUpDown, Globe, Search, Zap } from "lucide-react";
import Link from "next/link";
import { cn } from "@/lib/utils";
import type { SiteWorkbenchSummary } from "@/lib/api";

const STORAGE_KEY = "indexflow:lastSiteId";

function dotFor(row: SiteWorkbenchSummary) {
  if (row.activity?.running) return "bg-amber-400 animate-pulse";
  if (row.blocked > 0) return "bg-rose-400";
  if (row.submitted > 0 && row.pending === 0) return "bg-emerald-400";
  return "bg-slate-500";
}

export function SiteSwitcher({
  sites,
  selectedId,
}: {
  sites: SiteWorkbenchSummary[];
  selectedId: number | null;
}) {
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  const selected = useMemo(
    () => sites.find((s) => s.site.id === selectedId) ?? null,
    [sites, selectedId]
  );

  const filtered = useMemo(() => {
    const term = q.trim().toLowerCase();
    if (!term) return sites;
    return sites.filter((r) => r.site.domain.toLowerCase().includes(term));
  }, [sites, q]);

  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", onClick);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const go = (id: number) => {
    try {
      localStorage.setItem(STORAGE_KEY, String(id));
    } catch {}
    setOpen(false);
    router.push(`/site/${id}/dashboard`);
  };

  if (sites.length === 0) {
    return (
      <div className="mx-3 mb-3 p-3 rounded-xl border border-dashed border-slate-700/60 bg-slate-900/40">
        <div className="flex items-center gap-2 text-xs text-slate-500 mb-2">
          <Globe className="w-3.5 h-3.5" />
          No sites yet
        </div>
        <Link
          href="/sites/"
          className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs bg-indigo-600 hover:bg-indigo-500 text-white"
        >
          <Zap className="w-3 h-3" />
          Create first site
        </Link>
      </div>
    );
  }

  return (
    <div ref={ref} className="relative mx-3 mb-3">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "w-full flex items-center gap-2.5 px-3 py-2.5 rounded-xl border text-left transition",
          open
            ? "bg-slate-900 border-indigo-500/40 ring-2 ring-indigo-500/20"
            : "bg-slate-900/60 border-slate-800 hover:border-slate-700 hover:bg-slate-900"
        )}
      >
        <span className={cn("w-2 h-2 rounded-full shrink-0", selected ? dotFor(selected) : "bg-slate-600")} />
        <span className="min-w-0 flex-1">
          <span className="block text-sm font-medium text-white truncate">
            {selected ? selected.site.domain : "Select site"}
          </span>
          <span className="block text-[11px] text-slate-500 truncate">
            {selected
              ? `${selected.url_total} URLs · ${selected.pending} pending`
              : `${sites.length} sites`}
          </span>
        </span>
        <ChevronsUpDown className="w-4 h-4 text-slate-500 shrink-0" />
      </button>

      {open && (
        <div className="absolute left-0 right-0 mt-2 rounded-xl border border-slate-800 bg-slate-900 shadow-2xl overflow-hidden z-40">
          <div className="p-2 border-b border-slate-800">
            <div className="flex items-center gap-2 px-2.5 py-2 rounded-lg bg-slate-950 border border-slate-800">
              <Search className="w-3.5 h-3.5 text-slate-500" />
              <input
                autoFocus
                value={q}
                onChange={(e) => setQ(e.target.value)}
                placeholder="Search domain…"
                className="flex-1 bg-transparent text-sm text-slate-200 placeholder:text-slate-600 outline-none"
              />
            </div>
          </div>
          <div className="max-h-64 overflow-y-auto py-1">
            {filtered.length === 0 ? (
              <div className="px-3 py-6 text-center text-xs text-slate-500">No matches</div>
            ) : (
              filtered.map((row) => (
                <button
                  key={row.site.id}
                  type="button"
                  onClick={() => go(row.site.id)}
                  className={cn(
                    "w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-slate-800/60 transition",
                    row.site.id === selectedId && "bg-indigo-600/15"
                  )}
                >
                  <span className={cn("w-2 h-2 rounded-full shrink-0", dotFor(row))} />
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm text-slate-100 truncate">{row.site.domain}</span>
                    <span className="block text-[11px] text-slate-500">
                      {row.url_total} · P{row.pending} S{row.submitted} B{row.blocked}
                    </span>
                  </span>
                  {row.activity?.running && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded-full border border-amber-700/50 bg-amber-950/50 text-amber-300">
                      Running
                    </span>
                  )}
                </button>
              ))
            )}
          </div>
          <div className="px-3 py-2 border-t border-slate-800 flex items-center justify-between">
            <Link href="/sites/" className="text-xs text-indigo-400 hover:underline" onClick={() => setOpen(false)}>
              Manage sites →
            </Link>
            <span className="text-[11px] text-slate-600">{filtered.length}/{sites.length}</span>
          </div>
        </div>
      )}
    </div>
  );
}

export function getLastSiteId(): number | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const n = Number(raw);
    return Number.isFinite(n) && n > 0 ? n : null;
  } catch {
    return null;
  }
}
