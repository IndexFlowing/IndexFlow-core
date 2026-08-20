"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { ExternalLink, RotateCw } from "lucide-react";
import { StatusBadge } from "@/components/StatusBadge";
import { UrlDiagnosticsDrawer } from "@/components/UrlDiagnosticsDrawer";
import { api, LocaleCount, PathPrefixCount, Sitemap, UrlDiagnostic } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

const PAGE_SIZE = 40;

export default function SitemapsClient() {
  const params = useParams<{ id: string }>();
  const id = Number(Array.isArray(params?.id) ? params.id[0] : params?.id);
  const [sitemaps, setSitemaps] = useState<Sitemap[]>([]);
  const [urls, setUrls] = useState<UrlDiagnostic[]>([]);
  const [total, setTotal] = useState(0);
  const [locales, setLocales] = useState<LocaleCount[]>([]);
  const [prefixes, setPrefixes] = useState<PathPrefixCount[]>([]);
  const [localeFilter, setLocaleFilter] = useState("");
  const [prefixFilter, setPrefixFilter] = useState("");
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openUrl, setOpenUrl] = useState<UrlDiagnostic | null>(null);

  const load = useCallback(async () => {
    try {
      const [sms, loc, pre, table] = await Promise.all([
        api.listSitemaps(id),
        api.listLocales(id, prefixFilter || undefined),
        api.listPathPrefixes(id, localeFilter || undefined),
        api.listDiagnostics(id, {
          locale: localeFilter || undefined,
          path_prefix: prefixFilter || undefined,
          page,
          limit: PAGE_SIZE,
        }),
      ]);
      setSitemaps(sms);
      setLocales(loc);
      setPrefixes(pre);
      setUrls(table.items);
      setTotal(table.total);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, [id, localeFilter, prefixFilter, page]);

  useEffect(() => {
    load();
  }, [load]);

  const sync = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.syncSitemap(id);
      setMsg(r.message || `Done (tasks=${r.tasks_created})`);
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Sync failed");
    } finally {
      setBusy(false);
    }
  };

  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center gap-3">
        <button
          onClick={sync}
          disabled={busy}
          className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50"
        >
          <RotateCw className={`w-4 h-4 ${busy ? "animate-spin" : ""}`} />
          Sync sitemap
        </button>
        <p className="text-xs text-slate-500">Source-only · does not enqueue SEO or submit workers.</p>
        {msg && <span className="text-xs text-slate-400 border border-slate-800 bg-slate-900 px-2 py-1 rounded-lg">{msg}</span>}
      </div>
      {error && <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-sm text-rose-300">{error}</div>}

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <h2 className="font-semibold text-white mb-3">Sitemaps ({sitemaps.length})</h2>
        {sitemaps.length === 0 ? (
          <div className="text-sm text-slate-500 border border-dashed border-slate-800 rounded-xl py-8 text-center">No sitemaps. Add one in Site credentials or sync now.</div>
        ) : (
          <div className="space-y-2">
            {sitemaps.map((sm) => (
              <div key={sm.id} className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 p-3 rounded-xl bg-slate-950/50 border border-slate-800/80">
                <div className="min-w-0">
                  <a href={sm.url} target="_blank" rel="noreferrer" className="text-sm text-indigo-300 hover:underline break-all inline-flex items-center gap-1">
                    {sm.url}
                    <ExternalLink className="w-3 h-3 shrink-0" />
                  </a>
                  <div className="text-[11px] text-slate-500 mt-1">
                    Last sync {formatDate(sm.last_sync_at)}
                    {sm.last_error ? ` · ${sm.last_error}` : ""}
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <StatusBadge status={sm.type} />
                  <StatusBadge status={sm.status} />
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <div className="flex flex-wrap gap-2 mb-4">
          <select value={localeFilter} onChange={(e) => { setPage(1); setLocaleFilter(e.target.value); }} className="bg-slate-950 border border-slate-800 rounded-lg px-3 py-1.5 text-sm text-slate-200">
            <option value="">All locales</option>
            {locales.map((l) => <option key={l.locale} value={l.locale}>{l.locale} ({l.count})</option>)}
          </select>
          <select value={prefixFilter} onChange={(e) => { setPage(1); setPrefixFilter(e.target.value); }} className="bg-slate-950 border border-slate-800 rounded-lg px-3 py-1.5 text-sm text-slate-200">
            <option value="">All path prefixes</option>
            {prefixes.map((p) => <option key={p.path_prefix} value={p.path_prefix}>{p.path_prefix} ({p.count})</option>)}
          </select>
          <span className="text-xs text-slate-500 self-center">{formatNumber(total)} URLs · page {page}/{totalPages}</span>
        </div>
        {urls.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">No URLs match this filter. Sync the sitemap first.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b border-slate-800">
                  <th className="pb-2 pr-3 font-medium">URL</th>
                  <th className="pb-2 pr-3 font-medium">Locale</th>
                  <th className="pb-2 pr-3 font-medium">Prefix</th>
                  <th className="pb-2 pr-3 font-medium">Sitemap priority</th>
                  <th className="pb-2 pr-3 font-medium">Lastmod</th>
                  <th className="pb-2 font-medium">First seen</th>
                </tr>
              </thead>
              <tbody>
                {urls.map((u) => (
                  <tr key={u.id} onClick={() => setOpenUrl(u)} className="border-b border-slate-800/50 hover:bg-slate-900/50 cursor-pointer">
                    <td className="py-2.5 pr-3 max-w-sm"><span className="text-indigo-300 break-all text-xs font-mono">{u.url}</span></td>
                    <td className="py-2.5 pr-3 text-xs text-slate-300 font-mono">{u.locale}</td>
                    <td className="py-2.5 pr-3 text-xs text-slate-400 font-mono">{u.path_prefix}</td>
                    <td className="py-2.5 pr-3 text-xs text-slate-400">{u.sitemap_priority ?? "—"}</td>
                    <td className="py-2.5 pr-3 text-xs text-slate-500">{formatDate(u.sitemap_lastmod)}</td>
                    <td className="py-2.5 text-xs text-slate-500">{formatDate(u.updated_at)}</td>
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

      {openUrl && <UrlDiagnosticsDrawer key={openUrl.id} url={openUrl} mode="source" onClose={() => setOpenUrl(null)} onUpdated={load} />}
    </div>
  );
}
