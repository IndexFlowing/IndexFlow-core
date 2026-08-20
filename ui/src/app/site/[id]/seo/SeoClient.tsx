"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { FileSearch, ScanSearch } from "lucide-react";
import { StatCard } from "@/components/StatCard";
import { UrlDiagnosticsDrawer } from "@/components/UrlDiagnosticsDrawer";
import { api, SeoStats, UrlDiagnostic } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

const PAGE_SIZE = 40;

export default function SeoClient() {
  const params = useParams<{ id: string }>();
  const id = Number(Array.isArray(params?.id) ? params.id[0] : params?.id);
  const [seoStats, setSeoStats] = useState<SeoStats | null>(null);
  const [urls, setUrls] = useState<UrlDiagnostic[]>([]);
  const [total, setTotal] = useState(0);
  const [seoChecked, setSeoChecked] = useState<boolean | undefined>(undefined);
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openUrl, setOpenUrl] = useState<UrlDiagnostic | null>(null);

  const load = useCallback(async () => {
    try {
      const [seo, table] = await Promise.all([
        api.getSeoStats(id).catch(() => null),
        api.listDiagnostics(id, { seo_checked: seoChecked, page, limit: PAGE_SIZE }),
      ]);
      if (seo) setSeoStats(seo);
      setUrls(table.items);
      setTotal(table.total);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, [id, seoChecked, page]);

  useEffect(() => {
    load();
  }, [load]);

  const run = async (key: string, fn: () => Promise<{ message?: string; tasks_created?: number }>) => {
    setBusy(key);
    setMsg(null);
    try {
      const r = await fn();
      setMsg(r.message || `Done (tasks=${r.tasks_created ?? 0})`);
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Action failed");
    } finally {
      setBusy(null);
    }
  };

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap gap-2">
        <button onClick={() => run("full", () => api.seoAuditFull(id))} disabled={!!busy} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-50">
          <ScanSearch className={`w-4 h-4 ${busy === "full" ? "animate-pulse" : ""}`} />
          Run Full SEO Audit
        </button>
        <button onClick={() => run("unchecked", () => api.seoAuditUnchecked(id))} disabled={!!busy} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50">
          <FileSearch className="w-4 h-4" />
          Audit Unchecked
        </button>
        <span className="text-xs text-slate-500 self-center">Standalone scanner — does not submit to Bing or Google.</span>
        {msg && <span className="text-xs text-slate-400 border border-slate-800 bg-slate-900 px-2 py-1 rounded-lg">{msg}</span>}
      </div>
      {error && <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-sm text-rose-300">{error}</div>}

      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
        <button type="button" className="text-left" onClick={() => { setPage(1); setSeoChecked(undefined); }}>
          <StatCard label="Checked" value={formatNumber(seoStats?.checked ?? 0)} hint="last_checked_at set" accent={seoChecked === undefined ? "text-white" : undefined} />
        </button>
        <button type="button" className="text-left" onClick={() => { setPage(1); setSeoChecked((v) => (v === false ? undefined : false)); }}>
          <StatCard label="Unchecked" value={formatNumber(seoStats?.unchecked ?? 0)} hint="Never scanned" accent="text-amber-300" />
        </button>
        <StatCard label="Blocked" value={formatNumber(seoStats?.blocked ?? 0)} hint="Failed quality gate" accent="text-rose-400" />
      </div>

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-white text-sm">Quality gate details</h2>
          <span className="text-xs text-slate-500">{formatNumber(total)} rows · page {page}/{totalPages}</span>
        </div>
        {urls.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">No URLs for this filter.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b border-slate-800">
                  <th className="pb-2 pr-3 font-medium">URL</th>
                  <th className="pb-2 pr-3 font-medium">HTTP</th>
                  <th className="pb-2 pr-3 font-medium">Title</th>
                  <th className="pb-2 pr-3 font-medium">Block / diagnosis</th>
                  <th className="pb-2 font-medium">Last checked</th>
                </tr>
              </thead>
              <tbody>
                {urls.map((u) => (
                  <tr key={u.id} onClick={() => setOpenUrl(u)} className="border-b border-slate-800/50 hover:bg-slate-900/50 cursor-pointer">
                    <td className="py-2.5 pr-3 max-w-xs"><span className="text-indigo-300 break-all text-xs font-mono">{u.url}</span></td>
                    <td className="py-2.5 pr-3 text-xs font-mono text-slate-400">{u.last_http_status ?? "—"}</td>
                    <td className="py-2.5 pr-3 text-xs text-slate-300 max-w-[220px] truncate" title={u.page_title || ""}>{u.page_title || "—"}</td>
                    <td className="py-2.5 pr-3 text-xs text-rose-400 max-w-[240px] truncate" title={u.block_reason || ""}>{u.block_reason || "—"}</td>
                    <td className="py-2.5 text-xs text-slate-500">{formatDate(u.last_checked_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <div className="flex items-center justify-end gap-2 mt-4">
          <button disabled={page <= 1} onClick={() => setPage((p) => Math.max(1, p - 1))} className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40">Previous</button>
          <button disabled={page >= totalPages} onClick={() => setPage((p) => p + 1)} className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40">Next</button>
        </div>
      </section>

      {seoStats && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
            <h3 className="text-sm font-medium text-white mb-3">HTTP status breakdown</h3>
            {(seoStats.http_status.length ?? 0) === 0 ? <p className="text-xs text-slate-500">No scans yet.</p> : (
              <div className="space-y-2">
                {seoStats.http_status.slice(0, 8).map((h) => (
                  <div key={String(h.http_status)} className="flex justify-between text-xs text-slate-400">
                    <span>{h.http_status ?? "—"}</span>
                    <span className="font-mono">{h.count}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
            <h3 className="text-sm font-medium text-white mb-3">Blocked reasons</h3>
            {(seoStats.block_reasons.length ?? 0) === 0 ? <p className="text-xs text-slate-500">No blocked URLs.</p> : (
              <div className="space-y-2">
                {seoStats.block_reasons.slice(0, 8).map((r) => (
                  <div key={r.reason} className="flex justify-between text-xs text-slate-400 gap-4">
                    <span className="truncate" title={r.reason}>{r.reason}</span>
                    <span className="font-mono shrink-0">{r.count}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {openUrl && <UrlDiagnosticsDrawer key={openUrl.id} url={openUrl} mode="seo" onClose={() => setOpenUrl(null)} onUpdated={load} />}
    </div>
  );
}
