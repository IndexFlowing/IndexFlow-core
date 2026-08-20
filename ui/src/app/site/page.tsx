"use client";

import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import {
  ArrowLeft,
  ExternalLink,
  FileSearch,
  Globe,
  KeyRound,
  Radar,
  RefreshCw,
  RotateCw,
  ScanSearch,
  Send,
} from "lucide-react";
import { Shell } from "@/components/Shell";
import { StatCard } from "@/components/StatCard";
import { StatusBadge } from "@/components/StatusBadge";
import { UrlDiagnosticsDrawer } from "@/components/UrlDiagnosticsDrawer";
import { ProviderStatusBadge } from "@/components/ProviderStatus";
import {
  api,
  IndexMonitorStats,
  LocaleCount,
  PathPrefixCount,
  SeoStats,
  SiteActivity,
  SiteDetail,
  Sitemap,
  SiteUrlStats,
  UrlDiagnostic,
} from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

const PAGE_SIZE = 40;

type WorkbenchTab = "assets" | "seo" | "submit" | "index";

const TABS: { id: WorkbenchTab; label: string }[] = [
  { id: "assets", label: "Sitemap Assets" },
  { id: "seo", label: "SEO Quality" },
  { id: "submit", label: "Engine Submissions" },
  { id: "index", label: "Index Monitoring" },
];

function parseTab(raw: string | null): WorkbenchTab {
  if (raw === "seo" || raw === "submit" || raw === "index" || raw === "assets") {
    return raw;
  }
  return "assets";
}

function SiteWorkbenchInner() {
  const params = useSearchParams();
  const router = useRouter();
  const id = Number(params.get("id") || 0);
  const tab = parseTab(params.get("tab"));

  const [detail, setDetail] = useState<SiteDetail | null>(null);
  const [activity, setActivity] = useState<SiteActivity | null>(null);
  const [stats, setStats] = useState<SiteUrlStats | null>(null);
  const [sitemaps, setSitemaps] = useState<Sitemap[]>([]);
  const [urls, setUrls] = useState<UrlDiagnostic[]>([]);
  const [urlTotal, setUrlTotal] = useState(0);
  const [locales, setLocales] = useState<LocaleCount[]>([]);
  const [prefixes, setPrefixes] = useState<PathPrefixCount[]>([]);
  const [seoStats, setSeoStats] = useState<SeoStats | null>(null);
  const [indexStats, setIndexStats] = useState<IndexMonitorStats | null>(null);

  const [statusFilter, setStatusFilter] = useState("");
  const [localeFilter, setLocaleFilter] = useState("");
  const [prefixFilter, setPrefixFilter] = useState("");
  const [seoChecked, setSeoChecked] = useState<boolean | undefined>(undefined);
  const [indexFilter, setIndexFilter] = useState("");
  const [page, setPage] = useState(1);

  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [showCreds, setShowCreds] = useState(false);
  const [openUrl, setOpenUrl] = useState<UrlDiagnostic | null>(null);

  const [indexnowKey, setIndexnowKey] = useState("");
  const [googleJson, setGoogleJson] = useState("");
  const [credDirty, setCredDirty] = useState(false);
  const [savingCred, setSavingCred] = useState(false);
  const credDirtyRef = useRef(false);

  const setTab = (next: WorkbenchTab) => {
    const q = new URLSearchParams(params.toString());
    q.set("tab", next);
    router.replace(`/site/?${q.toString()}`);
    setPage(1);
  };

  const load = useCallback(async () => {
    if (!id) return;
    try {
      const [d, sms, st, loc, pre, table, seo, idx] = await Promise.all([
        api.getSite(id),
        api.listSitemaps(id),
        api.getSiteStats(id, {
          locale: localeFilter || undefined,
          path_prefix: prefixFilter || undefined,
        }),
        api.listLocales(id, prefixFilter || undefined),
        api.listPathPrefixes(id, localeFilter || undefined),
        api.listDiagnostics(id, {
          status: statusFilter || undefined,
          locale: localeFilter || undefined,
          path_prefix: prefixFilter || undefined,
          page,
          limit: PAGE_SIZE,
          seo_checked: seoChecked,
          google_index_status: indexFilter || undefined,
        }),
        api.getSeoStats(id).catch(() => null),
        api.getIndexStats(id).catch(() => null),
      ]);
      setDetail(d);
      setActivity(d.activity);
      setSitemaps(sms);
      setStats(st);
      setLocales(loc);
      setPrefixes(pre);
      setUrls(table.items);
      setUrlTotal(table.total);
      setError(null);
      if (seo) setSeoStats(seo);
      if (idx) setIndexStats(idx);
      if (!credDirtyRef.current) {
        setIndexnowKey(d.site.indexnow_key || "");
        setGoogleJson(d.site.google_service_account_json || "");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, [id, statusFilter, localeFilter, prefixFilter, page, seoChecked, indexFilter]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- async fetch; setState after await
    void load();
  }, [load]);

  useEffect(() => {
    const ms = activity?.running ? 3000 : 10000;
    const t = setInterval(load, ms);
    return () => clearInterval(t);
  }, [load, activity?.running]);

  const run = async (
    key: string,
    fn: () => Promise<{ message?: string; tasks_created?: number }>
  ) => {
    setBusy(key);
    setMsg(null);
    try {
      const res = await fn();
      setMsg(res.message || `Done (tasks=${res.tasks_created ?? 0})`);
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Action failed");
    } finally {
      setBusy(null);
    }
  };

  const saveCredentials = async () => {
    setSavingCred(true);
    setMsg(null);
    try {
      const res = await api.updateSiteCredentials(id, {
        indexnow_key: indexnowKey,
        google_service_account_json: googleJson,
        set_indexnow_key: true,
        set_google_service_account_json: true,
      });
      credDirtyRef.current = false;
      setCredDirty(false);
      setMsg(res.message || "Credentials saved");
      setIndexnowKey(res.site.indexnow_key || "");
      setGoogleJson(res.site.google_service_account_json || "");
      await load();
    } catch (e) {
      setMsg(e instanceof Error ? e.message : "Failed to save credentials");
    } finally {
      setSavingCred(false);
    }
  };

  const toggleStatus = (value: string) => {
    setPage(1);
    setStatusFilter((cur) => (cur === value ? "" : value));
  };

  if (!id) {
    return (
      <Shell title="Site workbench">
        <div className="text-rose-400 text-sm">Invalid site ID</div>
        <Link href="/sites/" className="text-indigo-400 text-sm mt-2 inline-block">
          ← Back to sites
        </Link>
      </Shell>
    );
  }

  const site = detail?.site;
  const totalPages = Math.max(1, Math.ceil(urlTotal / PAGE_SIZE));
  const conserved =
    (stats?.pending ?? 0) + (stats?.submitted ?? 0) + (stats?.blocked ?? 0) ===
    (stats?.url_total ?? 0);
  const bingSubmitted = stats?.bing_submitted_count ?? 0;
  const googleSubmitted = stats?.google_submitted_count ?? 0;
  const bingPending = stats?.bing_pending_count ?? 0;
  const googlePending = stats?.google_pending_count ?? 0;
  const urlTotalCount = stats?.url_total ?? 0;
  const quotaRemaining = detail?.google_quota_remaining ?? 0;
  const funnel = indexStats?.funnel;

  return (
    <Shell
      title={site?.domain || `Site #${id}`}
      subtitle="4 independent modules · sitemap · SEO gate · engine push · GSC index monitor"
      actions={
        <>
          <Link
            href="/"
            className="flex items-center gap-1.5 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-800 text-slate-300"
          >
            <ArrowLeft className="w-4 h-4" />
            Back
          </Link>
          <button
            onClick={() => {
              setLoading(true);
              load();
            }}
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-800 text-slate-300"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
            Refresh
          </button>
          <button
            onClick={() => setShowCreds((v) => !v)}
            className={`flex items-center gap-2 px-3 py-2 rounded-lg text-sm border ${
              showCreds
                ? "bg-indigo-600/20 border-indigo-500/40 text-indigo-200"
                : "bg-slate-900 border-slate-800 text-slate-300"
            }`}
          >
            <KeyRound className="w-4 h-4" />
            Credentials
          </button>
        </>
      }
    >
      {error && (
        <div className="mb-4 p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-sm">
          {error}
        </div>
      )}
      {msg && (
        <div className="mb-4 p-3 rounded-xl border border-slate-700 bg-slate-900 text-slate-300 text-sm">
          {msg}
        </div>
      )}

      <ActivityBanner activity={activity} />

      <div className="flex flex-wrap items-center gap-3 mb-6">
        {site && <StatusBadge status={site.status} />}
        {site && (
          <>
            <ProviderStatusBadge
              name="Bing"
              status={site.indexnow_status}
              error={site.indexnow_last_error}
            />
            <ProviderStatusBadge
              name="Google"
              status={site.google_status}
              error={site.google_last_error}
            />
          </>
        )}
        <span
          className={`text-[11px] ${conserved ? "text-emerald-500/80" : "text-rose-400"}`}
        >
          {conserved
            ? "Conserved: PENDING + SUBMITTED + BLOCKED = total"
            : "Mismatch: the three states do not sum to total"}
        </span>
      </div>

      {showCreds && (
        <CredentialsPanel
          id={id}
          site={site}
          indexnowKey={indexnowKey}
          googleJson={googleJson}
          credDirty={credDirty}
          savingCred={savingCred}
          busy={busy}
          setIndexnowKey={(v) => {
            setIndexnowKey(v);
            credDirtyRef.current = true;
            setCredDirty(true);
          }}
          setGoogleJson={(v) => {
            setGoogleJson(v);
            credDirtyRef.current = true;
            setCredDirty(true);
          }}
          saveCredentials={saveCredentials}
          run={run}
        />
      )}

      <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-6">
        <button type="button" onClick={() => toggleStatus("")} className="text-left">
          <StatCard
            label="Total"
            value={formatNumber(stats?.url_total ?? 0)}
            hint={statusFilter === "" ? "Current filter" : "Click to view all"}
            accent={statusFilter === "" ? "text-white" : undefined}
          />
        </button>
        <button type="button" onClick={() => toggleStatus("PENDING")} className="text-left">
          <StatCard
            label="Pending"
            value={formatNumber(stats?.pending ?? 0)}
            hint="PENDING"
            accent="text-amber-400"
          />
        </button>
        <button type="button" onClick={() => toggleStatus("SUBMITTED")} className="text-left">
          <StatCard
            label="Submitted"
            value={formatNumber(stats?.submitted ?? 0)}
            hint="All enabled engines succeeded"
            accent="text-emerald-400"
          />
        </button>
        <button type="button" onClick={() => toggleStatus("BLOCKED")} className="text-left">
          <StatCard
            label="Blocked"
            value={formatNumber(stats?.blocked ?? 0)}
            hint="BLOCKED"
            accent="text-rose-400"
          />
        </button>
      </div>

      <div className="flex flex-wrap gap-1 p-1 mb-6 rounded-xl border border-slate-800 bg-slate-900/50">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            className={`px-3 py-2 rounded-lg text-sm transition ${
              tab === t.id
                ? "bg-indigo-600 text-white"
                : "text-slate-400 hover:text-slate-200 hover:bg-slate-800"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "assets" && (
        <section className="space-y-6">
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => run("sync", () => api.syncSitemap(id))}
              disabled={!!busy}
              className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50"
            >
              <RotateCw className={`w-4 h-4 ${busy === "sync" ? "animate-spin" : ""}`} />
              Sync sitemap
            </button>
            <p className="text-xs text-slate-500 self-center">
              Sitemap sync is independent — it does not enqueue SEO or submit workers.
            </p>
          </div>

          <Facets
            locales={locales}
            prefixes={prefixes}
            localeFilter={localeFilter}
            prefixFilter={prefixFilter}
            setLocaleFilter={(v) => {
              setPage(1);
              setLocaleFilter(v);
            }}
            setPrefixFilter={(v) => {
              setPage(1);
              setPrefixFilter(v);
            }}
          />

          {sitemaps.length > 0 && (
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <h2 className="font-semibold text-white mb-3">Sitemaps</h2>
              <div className="space-y-2">
                {sitemaps.map((sm) => (
                  <div
                    key={sm.id}
                    className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 p-3 rounded-xl bg-slate-950/50 border border-slate-800/80"
                  >
                    <div className="min-w-0">
                      <a
                        href={sm.url}
                        target="_blank"
                        rel="noreferrer"
                        className="text-sm text-indigo-300 hover:underline break-all inline-flex items-center gap-1"
                      >
                        {sm.url}
                        <ExternalLink className="w-3 h-3 shrink-0" />
                      </a>
                      <div className="text-[11px] text-slate-500 mt-1">
                        Last sync {formatDate(sm.last_sync_at)}
                        {sm.last_error ? ` · Error: ${sm.last_error}` : ""}
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <StatusBadge status={sm.type} />
                      <StatusBadge status={sm.status} />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </section>
      )}

      {tab === "seo" && (
        <section className="space-y-6">
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => run("seo-full", () => api.seoAuditFull(id))}
              disabled={!!busy}
              className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-50"
            >
              <ScanSearch className={`w-4 h-4 ${busy === "seo-full" ? "animate-pulse" : ""}`} />
              Run Full SEO Audit
            </button>
            <button
              onClick={() => run("seo-unchecked", () => api.seoAuditUnchecked(id))}
              disabled={!!busy}
              className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50"
            >
              <FileSearch className="w-4 h-4" />
              Audit Unchecked
            </button>
            <p className="text-xs text-slate-500 self-center">
              Standalone scanner — does not submit to Bing or Google.
            </p>
          </div>

          <div className="grid grid-cols-2 lg:grid-cols-3 gap-3">
            <button
              type="button"
              className="text-left"
              onClick={() => {
                setPage(1);
                setSeoChecked(undefined);
              }}
            >
              <StatCard
                label="Checked"
                value={formatNumber(seoStats?.checked ?? 0)}
                hint="last_checked_at set"
                accent={seoChecked === undefined ? "text-white" : undefined}
              />
            </button>
            <button
              type="button"
              className="text-left"
              onClick={() => {
                setPage(1);
                setSeoChecked((v) => (v === false ? undefined : false));
              }}
            >
              <StatCard
                label="Unchecked"
                value={formatNumber(seoStats?.unchecked ?? 0)}
                hint="Never scanned"
                accent="text-amber-300"
              />
            </button>
            <StatCard
              label="Blocked"
              value={formatNumber(seoStats?.blocked ?? 0)}
              hint="Failed quality gate"
              accent="text-rose-400"
            />
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <h3 className="text-sm font-medium text-white mb-3">HTTP status breakdown</h3>
              {(seoStats?.http_status.length ?? 0) === 0 ? (
                <p className="text-xs text-slate-500">No scans yet.</p>
              ) : (
                <BarList
                  items={(seoStats?.http_status ?? []).map((h) => ({
                    label: String(h.http_status ?? "—"),
                    count: h.count,
                  }))}
                />
              )}
            </div>
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <h3 className="text-sm font-medium text-white mb-3">Blocked reasons</h3>
              {(seoStats?.block_reasons.length ?? 0) === 0 ? (
                <p className="text-xs text-slate-500">No blocked URLs.</p>
              ) : (
                <BarList
                  items={(seoStats?.block_reasons ?? []).map((r) => ({
                    label: r.reason,
                    count: r.count,
                  }))}
                />
              )}
            </div>
          </div>
        </section>
      )}

      {tab === "submit" && (
        <section className="space-y-6">
          <GoogleQuotaBar detail={detail} />
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-medium text-white">Bing IndexNow</h3>
                <span className="text-xs text-slate-500">No daily quota</span>
              </div>
              <p className="text-2xl font-semibold text-sky-300 mb-1">
                {formatNumber(bingSubmitted)} / {formatNumber(urlTotalCount)}
              </p>
              <p className="text-xs text-slate-500 mb-4">
                {bingPending > 0
                  ? `${formatNumber(bingPending)} pending`
                  : bingSubmitted > 0
                    ? "This engine is complete"
                    : "Not submitted"}
              </p>
              <button
                onClick={() => run("bing", () => api.startSubmitBing(id))}
                disabled={!!busy || bingPending <= 0}
                className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-sky-700 hover:bg-sky-600 text-white disabled:opacity-40"
              >
                <Send className={`w-4 h-4 ${busy === "bing" ? "animate-pulse" : ""}`} />
                Submit pending to Bing
              </button>
            </div>
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-medium text-white">Google Indexing API</h3>
                <span className="text-xs text-slate-500">
                  {formatNumber(quotaRemaining)} quota left
                </span>
              </div>
              <p className="text-2xl font-semibold text-indigo-300 mb-1">
                {formatNumber(googleSubmitted)} / {formatNumber(urlTotalCount)}
              </p>
              <p className="text-xs text-slate-500 mb-4">
                {googlePending > 0
                  ? `${formatNumber(googlePending)} pending · INDEXED pages are exempt`
                  : googleSubmitted > 0
                    ? "This engine is complete"
                    : "Not submitted"}
              </p>
              <button
                onClick={() => run("google", () => api.startSubmitGoogle(id))}
                disabled={!!busy || googlePending <= 0}
                className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-40"
              >
                <Globe className={`w-4 h-4 ${busy === "google" ? "animate-pulse" : ""}`} />
                Submit pending to Google
              </button>
            </div>
          </div>
        </section>
      )}

      {tab === "index" && (
        <section className="space-y-6">
          <div className="flex flex-wrap gap-2">
            <button
              onClick={() => run("gsc-sync", () => api.gscSyncAnalytics(id))}
              disabled={!!busy}
              className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-50"
            >
              <Radar className={`w-4 h-4 ${busy === "gsc-sync" ? "animate-pulse" : ""}`} />
              Sync Indexed URLs from GSC
            </button>
            <button
              onClick={() => run("gsc-inspect", () => api.gscInspectBatch(id))}
              disabled={!!busy}
              className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50"
            >
              <ScanSearch className="w-4 h-4" />
              Start GSC Inspection (Quota: {indexStats?.gsc_inspect_remaining ?? 2000}/day)
            </button>
          </div>
          <p className="text-xs text-slate-500">
            Search Analytics pages with impressions &gt; 0 are tagged INDEXED and skipped by the
            Google Indexing API quota. URL Inspection is capped at{" "}
            {indexStats?.gsc_inspect_quota_total ?? 2000}/day.
            {indexStats?.gsc_property_url
              ? ` Property: ${indexStats.gsc_property_url}.`
              : ""}
            {indexStats?.gsc_analytics_synced_at
              ? ` Last sync ${formatDate(indexStats.gsc_analytics_synced_at)}.`
              : ""}
          </p>
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
            <FunnelCard
              label="Confirmed Indexed"
              value={funnel?.indexed ?? 0}
              active={indexFilter === "INDEXED"}
              onClick={() => {
                setPage(1);
                setIndexFilter((v) => (v === "INDEXED" ? "" : "INDEXED"));
              }}
              accent="text-emerald-400"
            />
            <FunnelCard
              label="Crawled — Not Indexed"
              value={funnel?.crawled_not_indexed ?? 0}
              active={indexFilter === "CRAWLED_NOT_INDEXED"}
              onClick={() => {
                setPage(1);
                setIndexFilter((v) =>
                  v === "CRAWLED_NOT_INDEXED" ? "" : "CRAWLED_NOT_INDEXED"
                );
              }}
              accent="text-amber-300"
            />
            <FunnelCard
              label="Discovered — Not Indexed"
              value={funnel?.discovered_not_indexed ?? 0}
              active={indexFilter === "DISCOVERED_NOT_INDEXED"}
              onClick={() => {
                setPage(1);
                setIndexFilter((v) =>
                  v === "DISCOVERED_NOT_INDEXED" ? "" : "DISCOVERED_NOT_INDEXED"
                );
              }}
              accent="text-orange-300"
            />
            <FunnelCard
              label="Unknown"
              value={funnel?.unknown ?? 0}
              active={indexFilter === "UNKNOWN"}
              onClick={() => {
                setPage(1);
                setIndexFilter((v) => (v === "UNKNOWN" ? "" : "UNKNOWN"));
              }}
              accent="text-slate-300"
            />
          </div>
        </section>
      )}

      <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5 mt-6">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 mb-4">
          <h2 className="font-semibold text-white">
            URL details{" "}
            <span className="text-slate-500 font-normal text-sm">
              ({formatNumber(urlTotal)})
            </span>
          </h2>
          <span className="text-xs text-slate-500">
            Click a URL for deep diagnostics · Page {page}/{totalPages}
          </span>
        </div>

        {urls.length === 0 ? (
          <div className="text-center py-10 text-slate-500 text-sm border border-dashed border-slate-800 rounded-xl">
            No URLs match this filter. Sync the sitemap first.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs text-slate-500 border-b border-slate-800">
                  <th className="pb-2 pr-3 font-medium">URL</th>
                  <th className="pb-2 pr-3 font-medium">Locale</th>
                  <th className="pb-2 pr-3 font-medium">HTTP</th>
                  <th className="pb-2 pr-3 font-medium">Title</th>
                  <th className="pb-2 pr-3 font-medium">Lifecycle</th>
                  <th className="pb-2 pr-3 font-medium">Bing</th>
                  <th className="pb-2 pr-3 font-medium">Google</th>
                  <th className="pb-2 pr-3 font-medium">GSC</th>
                  <th className="pb-2 pr-3 font-medium">Block / diagnosis</th>
                  <th className="pb-2 font-medium">Updated</th>
                </tr>
              </thead>
              <tbody>
                {urls.map((u) => (
                  <tr
                    key={u.id}
                    onClick={() => setOpenUrl(u)}
                    className="border-b border-slate-800/50 hover:bg-slate-900/50 cursor-pointer"
                  >
                    <td className="py-2.5 pr-3 max-w-xs">
                      <span className="text-indigo-300 break-all text-xs font-mono">
                        {u.url}
                      </span>
                    </td>
                    <td className="py-2.5 pr-3 text-xs text-slate-300 font-mono">
                      {u.locale}
                    </td>
                    <td className="py-2.5 pr-3 text-slate-400 font-mono text-xs">
                      {u.last_http_status ?? "—"}
                    </td>
                    <td
                      className="py-2.5 pr-3 text-xs text-slate-300 max-w-[180px] truncate"
                      title={u.page_title || ""}
                    >
                      {u.page_title || "—"}
                    </td>
                    <td className="py-2.5 pr-3">
                      <div className="flex flex-wrap items-center gap-1">
                        <StatusBadge status={u.status} />
                        {u.queue_status && (
                          <span className="px-1.5 py-0.5 rounded text-[10px] border border-amber-700/50 bg-amber-950/60 text-amber-300">
                            {u.queue_status === "PROCESSING" ? "Running" : "Queued"}
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="py-2.5 pr-3">
                      <ChannelChip
                        status={u.bing_status}
                        error={u.bing_error}
                        at={u.bing_submitted_at}
                      />
                    </td>
                    <td className="py-2.5 pr-3">
                      <ChannelChip
                        status={u.google_status}
                        error={u.google_error}
                        at={u.google_submitted_at}
                      />
                    </td>
                    <td className="py-2.5 pr-3">
                      <StatusBadge status={u.google_index_status || "UNKNOWN"} />
                    </td>
                    <td
                      className="py-2.5 pr-3 text-xs text-rose-400/90 max-w-[200px] truncate"
                      title={u.block_reason || ""}
                    >
                      {u.block_reason || "—"}
                    </td>
                    <td className="py-2.5 text-xs text-slate-500 whitespace-nowrap">
                      {formatDate(u.updated_at)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}

        <div className="flex items-center justify-end gap-2 mt-4">
          <button
            disabled={page <= 1}
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40"
          >
            Previous
          </button>
          <button
            disabled={page >= totalPages}
            onClick={() => setPage((p) => p + 1)}
            className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40"
          >
            Next
          </button>
        </div>
      </section>

      {openUrl && (
        <UrlDiagnosticsDrawer
          key={openUrl.id}
          url={openUrl}
          onClose={() => setOpenUrl(null)}
          onUpdated={load}
        />
      )}
    </Shell>
  );
}

function Facets({
  locales,
  prefixes,
  localeFilter,
  prefixFilter,
  setLocaleFilter,
  setPrefixFilter,
}: {
  locales: LocaleCount[];
  prefixes: PathPrefixCount[];
  localeFilter: string;
  prefixFilter: string;
  setLocaleFilter: (v: string) => void;
  setPrefixFilter: (v: string) => void;
}) {
  return (
    <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
      <div className="space-y-3">
        <PillRow
          label="Locale"
          allCount={locales.reduce((s, x) => s + x.count, 0)}
          selected={localeFilter}
          onSelect={setLocaleFilter}
          items={locales.map((l) => ({
            key: l.locale,
            label: l.locale,
            count: l.count,
          }))}
        />
        <PillRow
          label="Path prefix"
          allCount={prefixes.reduce((s, x) => s + x.count, 0)}
          selected={prefixFilter}
          onSelect={setPrefixFilter}
          items={prefixes.map((p) => ({
            key: p.path_prefix,
            label: p.path_prefix,
            count: p.count,
          }))}
        />
      </div>
    </section>
  );
}

function BarList({ items }: { items: { label: string; count: number }[] }) {
  const max = Math.max(1, ...items.map((i) => i.count));
  return (
    <div className="space-y-2">
      {items.slice(0, 8).map((it) => (
        <div key={it.label}>
          <div className="flex justify-between text-[11px] text-slate-400 mb-0.5">
            <span className="truncate max-w-[70%]" title={it.label}>
              {it.label}
            </span>
            <span className="font-mono">{it.count}</span>
          </div>
          <div className="h-1.5 rounded-full bg-slate-800 overflow-hidden">
            <div
              className="h-full bg-indigo-500 rounded-full"
              style={{ width: `${Math.round((it.count / max) * 100)}%` }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

function FunnelCard({
  label,
  value,
  active,
  onClick,
  accent,
}: {
  label: string;
  value: number;
  active: boolean;
  onClick: () => void;
  accent?: string;
}) {
  return (
    <button type="button" onClick={onClick} className="text-left">
      <div
        className={`p-5 rounded-2xl border ${
          active ? "border-indigo-500/50 bg-indigo-950/30" : "border-slate-800 bg-slate-900/50"
        }`}
      >
        <div className="text-sm text-slate-400 mb-2">{label}</div>
        <div className={`text-3xl font-bold tracking-tight ${accent || "text-white"}`}>
          {formatNumber(value)}
        </div>
      </div>
    </button>
  );
}

function CredentialsPanel({
  id,
  site,
  indexnowKey,
  googleJson,
  credDirty,
  savingCred,
  busy,
  setIndexnowKey,
  setGoogleJson,
  saveCredentials,
  run,
}: {
  id: number;
  site: SiteDetail["site"] | undefined;
  indexnowKey: string;
  googleJson: string;
  credDirty: boolean;
  savingCred: boolean;
  busy: string | null;
  setIndexnowKey: (v: string) => void;
  setGoogleJson: (v: string) => void;
  saveCredentials: () => void;
  run: (
    key: string,
    fn: () => Promise<{ message?: string; tasks_created?: number }>
  ) => Promise<void>;
}) {
  return (
    <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5 mb-6">
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 mb-4">
        <div>
          <h2 className="font-semibold text-white">Search engine credentials</h2>
          <p className="text-xs text-slate-500 mt-0.5">
            After saving, run a test until the channel is Verified. The same service account
            must also be a user on the Search Console property for GSC sync.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={!!busy || savingCred}
            onClick={() =>
              run("tb", async () => {
                const r = await api.testBing(id);
                return { message: `[Bing] ${r.success ? "OK" : "Failed"}: ${r.message}` };
              })
            }
            className="px-2.5 py-1.5 rounded-lg text-xs border border-slate-700 text-slate-300 hover:bg-slate-800 disabled:opacity-50"
          >
            Test Bing
          </button>
          <button
            type="button"
            disabled={!!busy || savingCred}
            onClick={() =>
              run("tg", async () => {
                const r = await api.testGoogle(id);
                return { message: `[Google] ${r.success ? "OK" : "Failed"}: ${r.message}` };
              })
            }
            className="px-2.5 py-1.5 rounded-lg text-xs border border-slate-700 text-slate-300 hover:bg-slate-800 disabled:opacity-50"
          >
            Test Google
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <label className="block">
          <span className="block text-xs text-slate-400 mb-1">Bing IndexNow Key</span>
          <input
            value={indexnowKey}
            onChange={(e) => setIndexnowKey(e.target.value)}
            placeholder="Leave empty and save to clear"
            className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-sm text-slate-200 font-mono"
          />
        </label>
        <label className="block lg:col-span-2">
          <span className="block text-xs text-slate-400 mb-1">
            Google Service Account JSON
          </span>
          <textarea
            rows={5}
            value={googleJson}
            onChange={(e) => setGoogleJson(e.target.value)}
            placeholder="Paste the full JSON. Leave empty and save to clear."
            className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 font-mono"
          />
        </label>
      </div>

      {(site?.indexnow_last_error || site?.google_last_error) && (
        <div className="mt-3 space-y-1.5">
          {site?.indexnow_last_error && (
            <p className="text-xs text-rose-400/90 break-all">
              Bing error: {site.indexnow_last_error}
            </p>
          )}
          {site?.google_last_error && (
            <p className="text-xs text-rose-400/90 break-all">
              Google error: {site.google_last_error}
            </p>
          )}
        </div>
      )}

      <div className="flex items-center justify-between gap-3 mt-4">
        <p className="text-[11px] text-slate-500">
          {credDirty ? "Unsaved changes" : "In sync with the server"}
        </p>
        <button
          type="button"
          onClick={saveCredentials}
          disabled={savingCred || !credDirty}
          className="px-4 py-2 rounded-xl text-sm font-medium bg-indigo-600 hover:bg-indigo-500 text-white disabled:opacity-40"
        >
          {savingCred ? "Saving…" : "Save credentials"}
        </button>
      </div>
    </section>
  );
}

function PillRow({
  label,
  items,
  selected,
  onSelect,
  allCount,
}: {
  label: string;
  items: { key: string; label: string; count: number }[];
  selected: string;
  onSelect: (value: string) => void;
  allCount: number;
}) {
  return (
    <div>
      <div className="text-[11px] uppercase tracking-wide text-slate-500 mb-1.5">
        {label}
      </div>
      <div className="flex flex-wrap gap-1.5">
        <Pill active={selected === ""} onClick={() => onSelect("")} label={`All ${allCount}`} />
        {items.map((it) => (
          <Pill
            key={it.key}
            active={selected === it.key}
            onClick={() => onSelect(it.key)}
            label={`${it.label} ${it.count}`}
          />
        ))}
      </div>
    </div>
  );
}

function GoogleQuotaBar({ detail }: { detail: SiteDetail | null }) {
  const used = detail?.google_quota_used ?? 0;
  const total = detail?.google_quota_total || 200;
  const remaining = detail?.google_quota_remaining ?? Math.max(0, total - used);
  const pct = Math.min(100, Math.round((used / Math.max(1, total)) * 100));
  const warn = pct >= 90;
  return (
    <div className="rounded-xl border border-slate-800 bg-slate-900/50 px-4 py-3">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <span className="text-sm text-slate-300">
          This site&apos;s Google quota
          <span className="text-xs text-slate-500 ml-2">
            Rolling 24-hour window · INDEXED URLs do not consume it
          </span>
        </span>
        <span className={`text-xs font-mono ${warn ? "text-rose-400" : "text-slate-300"}`}>
          {used} / {total} · {remaining} left
        </span>
      </div>
      <div className="h-2 rounded-full bg-slate-800 overflow-hidden">
        <div
          className={`h-full rounded-full ${warn ? "bg-rose-500" : "bg-indigo-500"}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="text-[11px] text-slate-500 mt-2">
        {detail?.google_quota_next_free_at
          ? `Quota full. Next slot frees at ${formatDate(detail.google_quota_next_free_at)}`
          : "Each successful Indexing API submit occupies a slot for 24 hours. GSC-confirmed INDEXED URLs are exempt."}
      </p>
    </div>
  );
}

function ActivityBanner({ activity }: { activity: SiteActivity | null }) {
  const running = activity?.running;
  return (
    <div
      className={`mb-5 rounded-xl border px-4 py-3 text-sm flex flex-col sm:flex-row sm:items-center gap-2 ${
        running
          ? "border-amber-700/50 bg-amber-950/30 text-amber-100"
          : "border-slate-800 bg-slate-900/50 text-slate-300"
      }`}
    >
      <span
        className={`inline-flex items-center gap-1.5 font-medium ${
          running ? "text-amber-300" : "text-emerald-400"
        }`}
      >
        <span
          className={`w-2 h-2 rounded-full ${
            running ? "bg-amber-400 animate-pulse" : "bg-emerald-500"
          }`}
        />
        {running ? "Tasks running" : "Idle"}
      </span>
      <span className="text-xs sm:text-sm text-slate-300">
        {activity?.label || "Reading task status…"}
      </span>
      {running && (
        <span className="sm:ml-auto text-[11px] text-slate-400 font-mono">
          Sync {activity?.sync_processing ?? 0}/{activity?.sync_pending ?? 0}
          {" · "}
          Submit {activity?.submit_processing ?? 0}/{activity?.submit_pending ?? 0}
        </span>
      )}
    </div>
  );
}

function ChannelChip({
  status,
  error,
  at,
}: {
  status: string;
  error: string | null;
  at: string | null;
}) {
  const s = (status || "NONE").toUpperCase();
  const label =
    s === "SUBMITTED" ? "Submitted" : s === "FAILED" ? "Failed" : "Not submitted";
  const cls =
    s === "SUBMITTED"
      ? "bg-emerald-950/70 text-emerald-400 border-emerald-800/50"
      : s === "FAILED"
        ? "bg-rose-950/70 text-rose-400 border-rose-800/50"
        : "bg-slate-800/80 text-slate-400 border-slate-700";
  const title = [error, at ? formatDate(at) : ""].filter(Boolean).join(" · ");
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium border ${cls}`}
      title={title || undefined}
    >
      {label}
    </span>
  );
}

function Pill({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`px-2.5 py-1 rounded-full text-xs border transition ${
        active
          ? "bg-indigo-600/25 border-indigo-500/50 text-indigo-200"
          : "bg-slate-950 border-slate-800 text-slate-400 hover:border-slate-600 hover:text-slate-200"
      }`}
    >
      {label}
    </button>
  );
}

export default function SitePage() {
  return (
    <Suspense
      fallback={
        <Shell title="Site workbench">
          <div className="text-slate-500 text-sm">Loading…</div>
        </Shell>
      }
    >
      <SiteWorkbenchInner />
    </Suspense>
  );
}
