"use client";

import { ReactNode, useCallback, useEffect, useState } from "react";
import Link from "next/link";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  Gauge,
  Globe,
  Plus,
  RefreshCw,
  Server,
  Timer,
  Zap,
} from "lucide-react";
import { Shell } from "@/components/Shell";
import { StatusBadge } from "@/components/StatusBadge";
import { api, DashboardCounts, SiteWorkbenchSummary } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";
import { ProviderStatusBadge } from "@/components/ProviderStatus";

export default function DashboardPage() {
  const [counts, setCounts] = useState<DashboardCounts | null>(null);
  const [healthy, setHealthy] = useState(true);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [h, d] = await Promise.all([
        api.health().catch(() => null),
        api.dashboard(),
      ]);
      setHealthy(!!h);
      setCounts(d);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
      setHealthy(false);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
    const id = setInterval(load, 8000);
    return () => clearInterval(id);
  }, [load]);

  const cfg = counts?.config_info;
  const sites = counts?.sites ?? [];

  return (
    <Shell
      title="Sites overview"
      subtitle="Each site is an independent work unit · Open the workbench for conserved metrics"
      actions={
        <>
          <button
            onClick={() => {
              setLoading(true);
              load();
            }}
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-800 text-slate-300 hover:bg-slate-800"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
            Refresh
          </button>
          <Link
            href="/sites/"
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white"
          >
            <Plus className="w-4 h-4" />
            Add site
          </Link>
        </>
      }
    >
      {error && (
        <div className="mb-4 p-4 rounded-xl border border-rose-800/50 bg-rose-950/40 text-rose-300 text-sm">
          Cannot reach the backend: {error}
        </div>
      )}

      <div className="mb-6">
        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
          <div className="flex items-center gap-2 text-sm text-slate-400 mb-3">
            <Server className="w-4 h-4 text-indigo-400" />
            Runtime settings
            <span className="ml-auto flex items-center gap-1.5 text-xs">
              {healthy ? (
                <span className="text-emerald-400 flex items-center gap-1">
                  <CheckCircle2 className="w-3.5 h-3.5" /> Online
                </span>
              ) : (
                <span className="text-rose-400 flex items-center gap-1">
                  <AlertTriangle className="w-3.5 h-3.5" /> Offline
                </span>
              )}
            </span>
          </div>
          <div className="flex flex-wrap gap-2">
            <ParamChip
              icon={<Timer className="w-3.5 h-3.5" />}
              label="Submit pipeline"
              value={
                cfg
                  ? `${cfg.submit_worker_interval_secs}s / ${cfg.submit_worker_batch} URLs`
                  : "—"
              }
            />
            <ParamChip
              icon={<Activity className="w-3.5 h-3.5" />}
              label="Scheduler"
              value={cfg ? `every ${cfg.scheduler_interval_secs}s` : "—"}
            />
            <ParamChip
              icon={<Gauge className="w-3.5 h-3.5" />}
              label="Worker poll"
              value={cfg ? `${cfg.worker_poll_interval_secs}s` : "—"}
            />
            <ParamChip
              icon={<Zap className="w-3.5 h-3.5" />}
              label="Quality gate"
              value="Inline before submit"
            />
            <ParamChip
              icon={<Gauge className="w-3.5 h-3.5" />}
              label="Google quota"
              value={cfg ? `${cfg.google_daily_quota} / site · rolling 24h` : "per site · rolling 24h"}
            />
          </div>
        </div>
      </div>

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
        <div className="flex items-center justify-between mb-4">
          <h2 className="font-semibold text-white flex items-center gap-2">
            <Globe className="w-4 h-4 text-indigo-400" />
            Sites
          </h2>
          <Link href="/sites/" className="text-xs text-indigo-400 hover:underline">
            Add / manage →
          </Link>
        </div>

        {sites.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">
            No sites yet.
            <Link href="/sites/" className="text-indigo-400 ml-1 hover:underline">
              Add the first one
            </Link>
          </div>
        ) : (
          <div className="grid grid-cols-1 xl:grid-cols-2 gap-3">
            {sites.map((row) => (
              <SiteCard key={row.site.id} row={row} />
            ))}
          </div>
        )}
      </section>
    </Shell>
  );
}

function SiteCard({ row }: { row: SiteWorkbenchSummary }) {
  const { site } = row;
  const ok = row.pending + row.submitted + row.blocked === row.url_total;
  return (
    <Link
      href={`/site/?id=${site.id}`}
      className="block p-4 rounded-xl bg-slate-950/50 border border-slate-800 hover:border-indigo-500/40 transition"
    >
      <div className="flex items-start justify-between gap-3 mb-3">
        <div className="min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-medium text-white truncate">{site.domain}</span>
            <StatusBadge status={site.status} />
            {row.activity?.running && (
              <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] border border-amber-700/50 bg-amber-950/60 text-amber-300">
                <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
                Running
              </span>
            )}
          </div>
          <div className="mt-2 flex flex-wrap gap-1.5">
            <ProviderStatusBadge
              name="Bing"
              status={site.indexnow_status}
              error={site.indexnow_last_error}
              compact
            />
            <ProviderStatusBadge
              name="Google"
              status={site.google_status}
              error={site.google_last_error}
              compact
            />
          </div>
        </div>
        <span className="text-xs text-indigo-300 shrink-0">Open workbench →</span>
      </div>
      <div className="grid grid-cols-4 gap-2 text-center">
        <MiniStat label="Total" value={row.url_total} />
        <MiniStat label="Pending" value={row.pending} tone="text-amber-400" />
        <MiniStat label="Submitted" value={row.submitted} tone="text-emerald-400" />
        <MiniStat label="Blocked" value={row.blocked} tone="text-rose-400" />
      </div>
      <div className="grid grid-cols-2 gap-2 mt-2">
        <EngineProgress
          name="Bing"
          submitted={row.bing_submitted_count ?? 0}
          pending={row.bing_pending_count ?? 0}
          total={row.url_total}
        />
        <EngineProgress
          name="Google"
          submitted={row.google_submitted_count ?? 0}
          pending={row.google_pending_count ?? 0}
          total={row.url_total}
        />
      </div>
      <SiteQuotaBar row={row} />
      <p className={`text-[10px] mt-2 ${row.activity?.running ? "text-amber-400" : ok ? "text-slate-600" : "text-rose-400"}`}>
        {row.activity?.running
          ? row.activity.label
          : ok
            ? "Idle · 3-state conserved"
            : "Conservation mismatch"}
      </p>
    </Link>
  );
}

function SiteQuotaBar({ row }: { row: SiteWorkbenchSummary }) {
  const used = row.google_quota_used ?? 0;
  const total = row.google_quota_total || 200;
  const pct = Math.min(100, Math.round((used / Math.max(1, total)) * 100));
  const warn = pct >= 90;
  return (
    <div className="mt-3">
      <div className="flex items-center justify-between text-[10px] text-slate-500 mb-1">
        <span>Google rolling 24h</span>
        <span className={warn ? "text-rose-400" : "text-slate-400"}>
          {used} / {total}
        </span>
      </div>
      <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
        <div
          className={`h-full rounded-full ${warn ? "bg-rose-500" : "bg-indigo-500"}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="text-[10px] text-slate-600 mt-1">
        {row.google_quota_next_free_at
          ? `Next slot ${formatDate(row.google_quota_next_free_at)}`
          : "Each successful submit occupies a 24-hour slot"}
      </p>
    </div>
  );
}

function EngineProgress({
  name,
  submitted,
  pending,
  total,
}: {
  name: string;
  submitted: number;
  pending: number;
  total: number;
}) {
  const done = total > 0 && pending === 0 && submitted > 0;
  return (
    <div className="rounded-lg bg-slate-900/80 border border-slate-800/80 py-2 px-2">
      <div className="text-[10px] text-slate-500">{name} progress</div>
      <div
        className={`text-sm font-semibold mt-0.5 ${
          pending > 0 ? "text-amber-300" : done ? "text-emerald-400" : "text-slate-200"
        }`}
      >
        {formatNumber(submitted)} / {formatNumber(total)}
      </div>
      <div className="text-[10px] text-slate-500 mt-0.5">
        {pending > 0 ? `${formatNumber(pending)} pending` : done ? "Complete" : "Not submitted"}
      </div>
    </div>
  );
}

function MiniStat({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: string;
}) {
  return (
    <div className="rounded-lg bg-slate-900/80 border border-slate-800/80 py-2">
      <div className={`text-sm font-semibold ${tone || "text-slate-100"}`}>
        {formatNumber(value)}
      </div>
      <div className="text-[10px] text-slate-500 mt-0.5">{label}</div>
    </div>
  );
}

function ParamChip({
  icon,
  label,
  value,
}: {
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="inline-flex items-center gap-2 px-3 py-1.5 rounded-lg bg-slate-950 border border-slate-800 text-xs">
      <span className="text-slate-500">{icon}</span>
      <span className="text-slate-500">{label}</span>
      <span className="text-slate-200 font-medium">{value}</span>
    </div>
  );
}
