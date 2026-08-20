"use client";

import { useEffect, useState, useCallback } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { Activity, Globe, Timer, Zap } from "lucide-react";
import { StatCard } from "@/components/StatCard";
import { StatusBadge } from "@/components/StatusBadge";
import { api, SiteDetail, IndexMonitorStats } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

export default function DashboardClient() {
  const params = useParams<{ id: string }>();
  const id = Number(Array.isArray(params?.id) ? params.id[0] : params?.id);
  const [detail, setDetail] = useState<SiteDetail | null>(null);
  const [indexStats, setIndexStats] = useState<IndexMonitorStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      const [d, idx] = await Promise.all([
        api.getSite(id),
        api.getIndexStats(id).catch(() => null),
      ]);
      setDetail(d);
      if (idx) setIndexStats(idx);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    load();
    const t = setInterval(load, 8000);
    return () => clearInterval(t);
  }, [load]);

  if (loading && !detail) {
    return <div className="text-sm text-slate-500">Loading dashboard…</div>;
  }
  if (error) {
    return <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-sm text-rose-300">{error}</div>;
  }
  if (!detail) return null;

  const conserved = detail.pending + detail.submitted + detail.blocked === detail.url_total;
  const pct = detail.google_quota_total > 0 ? Math.round((detail.google_quota_used / detail.google_quota_total) * 100) : 0;
  const funnel = indexStats?.funnel;

  return (
    <div className="space-y-6">
      {detail.activity?.running && (
        <div className="flex items-center gap-2 px-4 py-3 rounded-xl border border-amber-700/40 bg-amber-950/40 text-sm text-amber-300">
          <span className="w-2 h-2 rounded-full bg-amber-400 animate-pulse" />
          {detail.activity.label}
          <span className="ml-auto text-xs text-amber-400/80">{detail.activity.phase}</span>
        </div>
      )}

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
        <Link href={`/site/${id}/sitemaps`} className="block">
          <StatCard label="Total" value={formatNumber(detail.url_total)} hint="Sitemap Assets →" />
        </Link>
        <Link href={`/site/${id}/seo`} className="block">
          <StatCard label="Pending" value={formatNumber(detail.pending)} hint="SEO Gate →" accent="text-amber-400" />
        </Link>
        <Link href={`/site/${id}/submissions`} className="block">
          <StatCard label="Submitted" value={formatNumber(detail.submitted)} hint="Engine Push →" accent="text-emerald-400" />
        </Link>
        <Link href={`/site/${id}/monitoring`} className="block">
          <StatCard label="Blocked" value={formatNumber(detail.blocked)} hint="Blocked" accent="text-rose-400" />
        </Link>
      </div>
      <div className={`text-xs ${conserved ? "text-emerald-500/80" : "text-rose-400"}`}>
        {conserved ? "Conserved: PENDING + SUBMITTED + BLOCKED = total" : "Mismatch: 3-state does not sum to total"}
        <span className="ml-2 text-slate-500 font-mono text-[11px]">
          {detail.pending} + {detail.submitted} + {detail.blocked} = {detail.url_total}
        </span>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
          <div className="text-xs text-slate-500 mb-1">Bing IndexNow</div>
          <div className="text-2xl font-semibold text-sky-300">{formatNumber(detail.bing_submitted_count)} / {formatNumber(detail.url_total)}</div>
          <div className="text-xs text-slate-500 mt-1">
            {detail.bing_pending_count > 0 ? `${formatNumber(detail.bing_pending_count)} pending` : detail.bing_submitted_count > 0 ? "Complete" : "Not submitted"}
          </div>
          <div className="text-[11px] text-slate-600 mt-2">No daily quota · provider status <StatusBadge status={detail.site.indexnow_status} /></div>
        </div>
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
          <div className="text-xs text-slate-500 mb-1">Google Indexing API</div>
          <div className="text-2xl font-semibold text-indigo-300">{formatNumber(detail.google_submitted_count)} / {formatNumber(detail.url_total)}</div>
          <div className="text-xs text-slate-500 mt-1">
            {detail.google_pending_count > 0 ? `${formatNumber(detail.google_pending_count)} pending · INDEXED exempt` : detail.google_submitted_count > 0 ? "Complete" : "Not submitted"}
          </div>
          <div className="mt-3">
            <div className="flex items-center justify-between text-[11px] text-slate-500 mb-1">
              <span>Rolling 24h</span>
              <span className={pct >= 90 ? "text-rose-400" : ""}>{detail.google_quota_used} / {detail.google_quota_total || 200}</span>
            </div>
            <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
              <div className={`h-full rounded-full ${pct >= 90 ? "bg-rose-500" : "bg-indigo-500"}`} style={{ width: `${Math.min(100, pct)}%` }} />
            </div>
            <div className="text-[11px] text-slate-600 mt-1">
              {detail.google_quota_next_free_at ? `Next slot ${formatDate(detail.google_quota_next_free_at)}` : "Each submit holds a 24h slot"}
            </div>
          </div>
        </div>
      </div>

      <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <h3 className="text-sm font-medium text-white mb-3">GSC funnel (Index Monitoring)</h3>
        {funnel ? (
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            <FunnelMini label="Indexed" value={funnel.indexed} tone="text-emerald-400" />
            <FunnelMini label="Crawled — not indexed" value={funnel.crawled_not_indexed} tone="text-amber-300" />
            <FunnelMini label="Discovered — not indexed" value={funnel.discovered_not_indexed} tone="text-orange-300" />
            <FunnelMini label="Unknown" value={funnel.unknown} tone="text-slate-300" />
          </div>
        ) : (
          <p className="text-xs text-slate-500">No GSC stats yet. Run Sync / Inspect in Monitoring.</p>
        )}
        <div className="mt-3 flex flex-wrap gap-2 text-xs">
          <Link href={`/site/${id}/monitoring`} className="text-indigo-400 hover:underline">Open Monitoring →</Link>
          <span className="text-slate-600">·</span>
          <Link href={`/site/${id}/sitemaps`} className="text-indigo-400 hover:underline">Sitemap Assets →</Link>
          <Link href={`/site/${id}/seo`} className="text-indigo-400 hover:underline">SEO Gate →</Link>
          <Link href={`/site/${id}/submissions`} className="text-indigo-400 hover:underline">Engine Push →</Link>
        </div>
      </div>

      <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4 flex flex-wrap gap-2 text-xs">
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-950 border border-slate-800 text-slate-400">
          <Timer className="w-3.5 h-3.5" />
          Activity: {detail.activity?.running ? detail.activity.label : "Idle"}
        </span>
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-950 border border-slate-800 text-slate-400">
          <Zap className="w-3.5 h-3.5" />
          Providers: Bing {detail.site.indexnow_status} · Google {detail.site.google_status}
        </span>
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-950 border border-slate-800 text-slate-400">
          <Globe className="w-3.5 h-3.5" />
          Domain {detail.site.domain}
        </span>
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-950 border border-slate-800 text-slate-500">
          <Activity className="w-3.5 h-3.5" />
          Sync + Submit decoupled
        </span>
      </div>
    </div>
  );
}

function FunnelMini({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return (
    <div className="rounded-xl bg-slate-950/50 border border-slate-800/80 p-4">
      <div className="text-xs text-slate-500">{label}</div>
      <div className={`text-xl font-semibold mt-1 ${tone || "text-white"}`}>{formatNumber(value)}</div>
    </div>
  );
}
