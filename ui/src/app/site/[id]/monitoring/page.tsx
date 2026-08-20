"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { Radar, ScanSearch } from "lucide-react";
import { StatusBadge } from "@/components/StatusBadge";
import { UrlDiagnosticsDrawer } from "@/components/UrlDiagnosticsDrawer";
import { api, IndexMonitorStats, UrlDiagnostic } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

const PAGE_SIZE = 40;

export default function MonitoringPage() {
  const params = useParams<{ id: string }>();
  const id = Number(Array.isArray(params?.id) ? params.id[0] : params?.id);
  const [stats, setStats] = useState<IndexMonitorStats | null>(null);
  const [urls, setUrls] = useState<UrlDiagnostic[]>([]);
  const [total, setTotal] = useState(0);
  const [indexFilter, setIndexFilter] = useState("");
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openUrl, setOpenUrl] = useState<UrlDiagnostic | null>(null);

  const load = useCallback(async () => {
    try {
      const [st, table] = await Promise.all([
        api.getIndexStats(id).catch(() => null),
        api.listDiagnostics(id, { google_index_status: indexFilter || undefined, page, limit: PAGE_SIZE }),
      ]);
      if (st) setStats(st);
      setUrls(table.items);
      setTotal(table.total);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, [id, indexFilter, page]);

  useEffect(() => {
    load();
  }, [load]);

  const run = async (key: string, fn: () => Promise<{ message?: string }>) => {
    setBusy(key);
    setMsg(null);
    try {
      const r = await fn();
      setMsg(r.message || "Done");
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Action failed");
    } finally {
      setBusy(null);
    }
  };

  const funnel = stats?.funnel;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap gap-2">
        <button onClick={() => run("sync", () => api.gscSyncAnalytics(id))} disabled={!!busy} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-50">
          <Radar className={`w-4 h-4 ${busy === "sync" ? "animate-pulse" : ""}`} />
          Sync Indexed URLs from GSC
        </button>
        <button onClick={() => run("inspect", () => api.gscInspectBatch(id))} disabled={!!busy} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50">
          <ScanSearch className="w-4 h-4" />
          Start GSC Inspection (Quota: {stats?.gsc_inspect_remaining ?? 2000}/day)
        </button>
        {msg && <span className="text-xs text-slate-400 border border-slate-800 bg-slate-900 px-2 py-1 rounded-lg self-center">{msg}</span>}
      </div>
      <p className="text-xs text-slate-500">
        Search Analytics pages with impressions &gt; 0 are tagged INDEXED and exempt from Google Indexing API quota. URL Inspection is capped at {stats?.gsc_inspect_quota_total ?? 2000}/day.
        {stats?.gsc_property_url ? ` Property: ${stats.gsc_property_url}.` : ""}{" "}
        {stats?.gsc_analytics_synced_at ? `Last sync ${formatDate(stats.gsc_analytics_synced_at)}.` : ""}
      </p>
      {error && <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-sm text-rose-300">{error}</div>}

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <FunnelCard label="Confirmed Indexed" value={funnel?.indexed ?? 0} active={indexFilter === "INDEXED"} onClick={() => { setPage(1); setIndexFilter((v) => (v === "INDEXED" ? "" : "INDEXED")); }} accent="text-emerald-400" />
        <FunnelCard label="Crawled — Not Indexed" value={funnel?.crawled_not_indexed ?? 0} active={indexFilter === "CRAWLED_NOT_INDEXED"} onClick={() => { setPage(1); setIndexFilter((v) => (v === "CRAWLED_NOT_INDEXED" ? "" : "CRAWLED_NOT_INDEXED")); }} accent="text-amber-300" />
        <FunnelCard label="Discovered — Not Indexed" value={funnel?.discovered_not_indexed ?? 0} active={indexFilter === "DISCOVERED_NOT_INDEXED"} onClick={() => { setPage(1); setIndexFilter((v) => (v === "DISCOVERED_NOT_INDEXED" ? "" : "DISCOVERED_NOT_INDEXED")); }} accent="text-orange-300" />
        <FunnelCard label="Unknown" value={funnel?.unknown ?? 0} active={indexFilter === "UNKNOWN"} onClick={() => { setPage(1); setIndexFilter((v) => (v === "UNKNOWN" ? "" : "UNKNOWN")); }} accent="text-slate-300" />
      </div>

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-white text-sm">GSC details <span className="text-slate-500 font-normal">({formatNumber(total)})</span></h2>
          <span className="text-xs text-slate-500">Page {page}/{totalPages}</span>
        </div>
        {urls.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">No URLs match this filter.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b border-slate-800">
                  <th className="pb-2 pr-3 font-medium">URL</th>
                  <th className="pb-2 pr-3 font-medium">GSC</th>
                  <th className="pb-2 pr-3 font-medium">Coverage</th>
                  <th className="pb-2 pr-3 font-medium">Last crawled</th>
                  <th className="pb-2 font-medium">Inspected</th>
                </tr>
              </thead>
              <tbody>
                {urls.map((u) => (
                  <tr key={u.id} onClick={() => setOpenUrl(u)} className="border-b border-slate-800/50 hover:bg-slate-900/50 cursor-pointer">
                    <td className="py-2.5 pr-3 max-w-sm"><span className="text-indigo-300 break-all text-xs font-mono">{u.url}</span></td>
                    <td className="py-2.5 pr-3"><StatusBadge status={u.google_index_status || "UNKNOWN"} /></td>
                    <td className="py-2.5 pr-3 text-xs text-slate-400 max-w-[200px] truncate" title={u.google_coverage_state || ""}>{u.google_coverage_state || "—"}</td>
                    <td className="py-2.5 pr-3 text-xs text-slate-500">{formatDate(u.google_last_crawled_at)}</td>
                    <td className="py-2.5 text-xs text-slate-500">{formatDate(u.google_inspected_at)}</td>
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

      {openUrl && <UrlDiagnosticsDrawer key={openUrl.id} url={openUrl} mode="gsc" onClose={() => setOpenUrl(null)} onUpdated={load} />}
    </div>
  );
}

function FunnelCard({ label, value, active, onClick, accent }: { label: string; value: number; active: boolean; onClick: () => void; accent?: string }) {
  return (
    <button type="button" onClick={onClick} className="text-left">
      <div className={`p-5 rounded-2xl border ${active ? "border-indigo-500/50 bg-indigo-950/30" : "border-slate-800 bg-slate-900/50"}`}>
        <div className="text-sm text-slate-400 mb-2">{label}</div>
        <div className={`text-3xl font-bold tracking-tight ${accent || "text-white"}`}>{formatNumber(value)}</div>
      </div>
    </button>
  );
}
