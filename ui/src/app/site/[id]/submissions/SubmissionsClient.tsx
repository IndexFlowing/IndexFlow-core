"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams, useSearchParams, useRouter } from "next/navigation";
import { Globe, Send } from "lucide-react";
import { StatusBadge } from "@/components/StatusBadge";
import { UrlDiagnosticsDrawer } from "@/components/UrlDiagnosticsDrawer";
import { api, SiteDetail, UrlDiagnostic } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

const PAGE_SIZE = 40;

function ChannelChip({ status, error, at }: { status: string; error: string | null; at: string | null }) {
  return (
    <div className="text-xs">
      <StatusBadge status={status || "NONE"} />
      {error && <div className="text-[11px] text-rose-400/80 truncate max-w-[160px]" title={error}>{error}</div>}
      {at && <div className="text-[11px] text-slate-500">{formatDate(at)}</div>}
    </div>
  );
}

export default function SubmissionsClient() {
  const params = useParams<{ id: string }>();
  const id = Number(Array.isArray(params?.id) ? params.id[0] : params?.id);
  const searchParams = useSearchParams();
  const router = useRouter();
  const engine = (searchParams.get("engine") === "google" ? "google" : "bing") as "bing" | "google";
  const setEngine = (e: "bing" | "google") => {
    const q = new URLSearchParams(searchParams.toString());
    q.set("engine", e);
    router.replace(`/site/${id}/submissions?${q.toString()}`);
  };

  const [detail, setDetail] = useState<SiteDetail | null>(null);
  const [urls, setUrls] = useState<UrlDiagnostic[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [openUrl, setOpenUrl] = useState<UrlDiagnostic | null>(null);
  const [drawerEngine, setDrawerEngine] = useState<"bing" | "google">("bing");

  const load = useCallback(async () => {
    try {
      const [d, table] = await Promise.all([
        api.getSite(id),
        api.listDiagnostics(id, { page, limit: PAGE_SIZE }),
      ]);
      setDetail(d);
      setUrls(table.items);
      setTotal(table.total);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, [id, page]);

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

  const filtered = urls.filter((u) => (engine === "bing" ? true : true)).filter((u) => {
    // Client-side engine focus is presentation-only; actual per-engine filtering comes from overlapping statuses
    // Keep all rows in table but highlight focused engine
    return true;
  });

  const bingPending = detail?.bing_pending_count ?? 0;
  const googlePending = detail?.google_pending_count ?? 0;
  const googleQuotaRemaining = detail?.google_quota_remaining ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const quotaPct = detail ? Math.round(((detail.google_quota_used) / Math.max(1, detail.google_quota_total || 200)) * 100) : 0;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap gap-2 p-1 rounded-xl border border-slate-800 bg-slate-900/50 w-fit">
        <button
          type="button"
          onClick={() => setEngine("bing")}
          className={`px-3 py-2 rounded-lg text-sm ${engine === "bing" ? "bg-sky-600 text-white" : "text-slate-400 hover:text-slate-200"}`}
        >
          Bing IndexNow
        </button>
        <button
          type="button"
          onClick={() => setEngine("google")}
          className={`px-3 py-2 rounded-lg text-sm ${engine === "google" ? "bg-indigo-600 text-white" : "text-slate-400 hover:text-slate-200"}`}
        >
          Google Indexing API
        </button>
      </div>

      {error && <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-sm text-rose-300">{error}</div>}

      {engine === "bing" ? (
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-medium text-white">Bing IndexNow</h3>
            <span className="text-xs text-slate-500">No daily quota</span>
          </div>
          <p className="text-2xl font-semibold text-sky-300 mb-1">{formatNumber(detail?.bing_submitted_count ?? 0)} / {formatNumber(detail?.url_total ?? 0)}</p>
          <p className="text-xs text-slate-500 mb-4">{bingPending > 0 ? `${formatNumber(bingPending)} pending` : "No pending — filtered view still shows all URLs for diagnosis"}</p>
          <button onClick={() => run("bing", () => api.startSubmitBing(id))} disabled={!!busy || bingPending <= 0} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-sky-700 hover:bg-sky-600 text-white disabled:opacity-40">
            <Send className={`w-4 h-4 ${busy === "bing" ? "animate-pulse" : ""}`} />
            Submit pending to Bing
          </button>
          {msg && <span className="ml-3 text-xs text-slate-400">{msg}</span>}
        </div>
      ) : (
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
          <div className="flex items-center justify-between mb-3">
            <h3 className="font-medium text-white">Google Indexing API</h3>
            <span className="text-xs text-slate-500">{formatNumber(googleQuotaRemaining)} quota left</span>
          </div>
          <p className="text-2xl font-semibold text-indigo-300 mb-1">{formatNumber(detail?.google_submitted_count ?? 0)} / {formatNumber(detail?.url_total ?? 0)}</p>
          <p className="text-xs text-slate-500 mb-3">{googlePending > 0 ? `${formatNumber(googlePending)} pending · INDEXED pages are exempt` : "No pending"}</p>
          <div className="mb-4">
            <div className="flex items-center justify-between text-[11px] text-slate-500 mb-1">
              <span>Rolling 24h</span>
              <span className={quotaPct >= 90 ? "text-rose-400" : ""}>{detail?.google_quota_used ?? 0} / {detail?.google_quota_total || 200}</span>
            </div>
            <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
              <div className={`h-full rounded-full ${quotaPct >= 90 ? "bg-rose-500" : "bg-indigo-500"}`} style={{ width: `${Math.min(100, quotaPct)}%` }} />
            </div>
            <div className="text-[11px] text-slate-600 mt-1">{detail?.google_quota_next_free_at ? `Next slot ${formatDate(detail.google_quota_next_free_at)}` : "—"}</div>
          </div>
          <button onClick={() => run("google", () => api.startSubmitGoogle(id))} disabled={!!busy || googlePending <= 0} className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-40">
            <Globe className={`w-4 h-4 ${busy === "google" ? "animate-pulse" : ""}`} />
            Submit pending to Google
          </button>
          {msg && <span className="ml-3 text-xs text-slate-400">{msg}</span>}
        </div>
      )}

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <div className="flex items-center justify-between mb-3">
          <h2 className="font-semibold text-white text-sm">
            {engine === "bing" ? "Bing" : "Google"} URL details{" "}
            <span className="text-slate-500 font-normal">({formatNumber(total)})</span>
          </h2>
          <span className="text-xs text-slate-500">Page {page}/{totalPages}</span>
        </div>
        {filtered.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">No URLs.</div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b border-slate-800">
                  <th className="pb-2 pr-3 font-medium">URL</th>
                  <th className="pb-2 pr-3 font-medium">{engine === "bing" ? "Bing" : "Google"}</th>
                  <th className="pb-2 font-medium">Updated</th>
                </tr>
              </thead>
              <tbody>
                {filtered.map((u) => (
                  <tr
                    key={u.id}
                    onClick={() => {
                      setDrawerEngine(engine);
                      setOpenUrl(u);
                    }}
                    className="border-b border-slate-800/50 hover:bg-slate-900/50 cursor-pointer"
                  >
                    <td className="py-2.5 pr-3 max-w-sm"><span className="text-indigo-300 break-all text-xs font-mono">{u.url}</span></td>
                    <td className="py-2.5 pr-3">
                      <ChannelChip
                        status={engine === "bing" ? u.bing_status : u.google_status}
                        error={engine === "bing" ? u.bing_error : u.google_error}
                        at={engine === "bing" ? u.bing_submitted_at : u.google_submitted_at}
                      />
                    </td>
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

      {openUrl && <UrlDiagnosticsDrawer key={`${openUrl.id}-${drawerEngine}`} url={openUrl} mode={drawerEngine} onClose={() => setOpenUrl(null)} onUpdated={load} />}
    </div>
  );
}
