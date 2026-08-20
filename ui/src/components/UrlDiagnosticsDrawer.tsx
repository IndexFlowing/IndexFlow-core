"use client";

import { useCallback, useEffect, useState } from "react";
import {
  CheckCircle2,
  Clock,
  ExternalLink,
  Globe,
  Loader2,
  RefreshCw,
  Send,
  X,
} from "lucide-react";
import { StatusBadge } from "@/components/StatusBadge";
import {
  api,
  RecheckResult,
  SubmitNowResult,
  UrlAnalysis,
  UrlDiagnostic,
} from "@/lib/api";
import { formatBytes, formatDate } from "@/lib/utils";

export type DrawerMode = "full" | "source" | "seo" | "bing" | "google" | "gsc";

export function UrlDiagnosticsDrawer({
  url,
  mode = "full",
  onClose,
  onUpdated,
}: {
  url: UrlDiagnostic;
  mode?: DrawerMode;
  onClose: () => void;
  onUpdated: () => void;
}) {
  const [analysis, setAnalysis] = useState<UrlAnalysis | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [flash, setFlash] = useState<string | null>(null);
  const [lastSubmit, setLastSubmit] = useState<SubmitNowResult | null>(null);
  const [lastRecheck, setLastRecheck] = useState<RecheckResult | null>(null);

  const load = useCallback(async () => {
    try {
      const a = await api.getUrlAnalysis(url.id);
      setAnalysis(a);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load analysis");
    } finally {
      setLoading(false);
    }
  }, [url.id]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- async fetch; setState after await
    void load();
  }, [load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const u = analysis?.url;
  const signals = analysis?.signals;
  const status = u?.status ?? url.status;
  const blockReason = u?.block_reason ?? url.block_reason;
  const pageUrl = u?.url ?? url.url;

  const recheck = async () => {
    setBusy("recheck");
    setFlash(null);
    try {
      const r = await api.recheckUrl(url.id);
      setLastRecheck(r);
      setFlash(
        r.passed
          ? "SEO re-check passed — BLOCKED status cleared if it was blocked."
          : `Still blocked: ${r.block_reason || "quality gate failed"}`
      );
      await load();
      onUpdated();
    } catch (e) {
      setFlash(e instanceof Error ? e.message : "Re-check failed");
    } finally {
      setBusy(null);
    }
  };

  const submit = async (provider: "bing" | "google") => {
    setBusy(provider);
    setFlash(null);
    try {
      const r = await api.submitUrlNow(url.id, provider);
      setLastSubmit(r);
      setFlash(r.message);
      await load();
      onUpdated();
    } catch (e) {
      setFlash(e instanceof Error ? e.message : "Submit failed");
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <button
        type="button"
        aria-label="Close diagnostics"
        className="absolute inset-0 bg-slate-950/60 backdrop-blur-[2px]"
        onClick={onClose}
      />
      <aside className="relative w-full max-w-xl h-full bg-slate-950 border-l border-slate-800 shadow-2xl overflow-y-auto">
        <header className="sticky top-0 z-10 bg-slate-950/95 border-b border-slate-800 px-5 py-4">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2 mb-2">
                <StatusBadge status={status} />
                {blockReason && (
                  <span className="text-[11px] text-rose-400 truncate max-w-[240px]" title={blockReason}>
                    {blockReason}
                  </span>
                )}
              </div>
              <a
                href={pageUrl}
                target="_blank"
                rel="noreferrer"
                className="text-sm text-indigo-300 hover:underline break-all font-mono inline-flex items-start gap-1"
              >
                {pageUrl}
                <ExternalLink className="w-3 h-3 shrink-0 mt-1" />
              </a>
            </div>
            <button
              type="button"
              onClick={onClose}
              className="p-1.5 rounded-lg border border-slate-800 text-slate-400 hover:text-white"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <div className="flex flex-wrap gap-2 mt-4">
            {mode !== "gsc" && (
              <button
                type="button"
                disabled={!!busy}
                onClick={recheck}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-900 border border-slate-700 text-slate-200 disabled:opacity-50"
              >
                {busy === "recheck" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="w-3.5 h-3.5" />
                )}
                Re-check SEO Now
              </button>
            )}
            {(mode === "full" || mode === "bing") && (
              <button
                type="button"
                disabled={!!busy || status === "BLOCKED"}
                onClick={() => submit("bing")}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-sky-950/70 border border-sky-800/60 text-sky-200 disabled:opacity-50"
              >
                {busy === "bing" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Send className="w-3.5 h-3.5" />
                )}
                Submit to Bing Now
              </button>
            )}
            {(mode === "full" || mode === "google") && (
              <button
                type="button"
                disabled={!!busy || status === "BLOCKED"}
                onClick={() => submit("google")}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-indigo-950/70 border border-indigo-800/60 text-indigo-200 disabled:opacity-50"
              >
                {busy === "google" ? (
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Globe className="w-3.5 h-3.5" />
                )}
                Submit to Google Now
              </button>
            )}
          </div>
        </header>

        <div className="px-5 py-4 space-y-4">
          {flash && (
            <div className="p-3 rounded-xl border border-slate-700 bg-slate-900 text-slate-300 text-xs">
              {flash}
            </div>
          )}
          {error && (
            <div className="p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-xs">
              {error}
            </div>
          )}
          {loading && (
            <div className="text-slate-500 text-sm flex items-center gap-2">
              <Loader2 className="w-4 h-4 animate-spin" />
              Loading diagnostics…
            </div>
          )}

          {signals && (
            <>
              {(mode === "full" || mode === "source" || mode === "seo") && (
                <>
                  <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                    <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-3">
                      Meta tags
                    </h3>
                    <SignalRow
                      label={`<title> · ${signals.title_chars} chars`}
                      value={signals.title}
                      warn={signals.title_chars === 0 || signals.title_chars > 60}
                    />
                    <SignalRow
                      label={`<meta name="description"> · ${signals.meta_description_chars} chars`}
                      value={signals.meta_description}
                      warn={
                        signals.meta_description_chars === 0 ||
                        signals.meta_description_chars > 160
                      }
                    />
                    <SignalRow label="<h1>" value={signals.h1} />
                  </section>

                  <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                    <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-3">
                      Directives & signals
                    </h3>
                    <SignalRow
                      label="<link rel=&quot;canonical&quot;>"
                      value={signals.canonical_url}
                      extra={
                        signals.canonical_matches == null
                          ? "No canonical declared"
                          : signals.canonical_matches
                            ? "Matches page URL"
                            : "Mismatch"
                      }
                      warn={signals.canonical_matches === false}
                    />
                    <SignalRow label="robots" value={signals.robots} />
                    {signals.hreflang.length > 0 ? (
                      <div className="mt-2 space-y-1">
                        <div className="text-[11px] text-slate-500">hreflang alternates</div>
                        {signals.hreflang.map((h, i) => (
                          <div key={`${h.lang}-${i}`} className="text-xs text-slate-300 font-mono break-all">
                            {h.lang}: {h.href}
                          </div>
                        ))}
                      </div>
                    ) : (
                      <SignalRow label="hreflang" value="—" />
                    )}
                  </section>

                  <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                    <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-3">
                      Network & timing
                    </h3>
                    <div className="grid grid-cols-3 gap-3">
                      <MiniStat
                        label="HTTP"
                        value={signals.http_status != null ? String(signals.http_status) : "—"}
                        ok={signals.http_status === 200}
                      />
                      <MiniStat
                        label="Latency"
                        value={
                          signals.response_time_ms != null
                            ? `${signals.response_time_ms} ms`
                            : "—"
                        }
                      />
                      <MiniStat
                        label="Payload"
                        value={formatBytes(signals.payload_bytes)}
                      />
                    </div>
                  </section>
                </>
              )}
              {mode === "source" && (
                <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                  <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-3">Source</h3>
                  <SignalRow label="URL" value={pageUrl} />
                  <SignalRow label="Locale" value={u?.locale ?? url.locale} />
                  <SignalRow label="Path prefix" value={u?.path_prefix ?? url.path_prefix} />
                </section>
              )}
            </>
          )}

          {analysis && (
            <>
              {(mode === "full" || mode === "gsc" || mode === "bing" || mode === "google") && (
                <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-4">
                  <h3 className="text-xs uppercase tracking-wide text-slate-500 mb-3">
                    Submission & GSC trail
                  </h3>
                  {(mode === "full" || mode === "gsc") && (
                    <div className="flex flex-wrap gap-2 mb-3">
                      <StatusBadge status={analysis.gsc.index_status || "UNKNOWN"} />
                      <span className="text-[11px] text-slate-500 inline-flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        Last crawl {formatDate(analysis.gsc.last_crawled_at)}
                      </span>
                      <span className="text-[11px] text-slate-500">
                        Inspected {formatDate(analysis.gsc.inspected_at)}
                      </span>
                    </div>
                  )}
                  {analysis.gsc.coverage_state && (mode === "full" || mode === "gsc") && (
                    <p className="text-xs text-slate-300 mb-3">coverageState: {analysis.gsc.coverage_state}</p>
                  )}
                  <div className="grid grid-cols-2 gap-2 text-xs mb-3">
                    {(mode === "full" || mode === "bing" || mode === "gsc") && (
                      <div className="rounded-lg border border-slate-800 bg-slate-950/60 p-2">
                        <div className="text-slate-500">Bing submit</div>
                        <div className="text-slate-200 mt-0.5">{u?.bing_status || "NONE"}</div>
                        {u?.bing_error && <div className="text-rose-400 mt-1 break-all">{u.bing_error}</div>}
                      </div>
                    )}
                    {(mode === "full" || mode === "google" || mode === "gsc") && (
                      <div className="rounded-lg border border-slate-800 bg-slate-950/60 p-2">
                        <div className="text-slate-500">Google submit</div>
                        <div className="text-slate-200 mt-0.5">{u?.google_status || "NONE"}</div>
                        {u?.google_error && <div className="text-rose-400 mt-1 break-all">{u.google_error}</div>}
                      </div>
                    )}
                  </div>
                </section>
              )}
              {(mode === "full" || mode === "seo" || mode === "bing" || mode === "google") && lastRecheck && (
                <LiveBox
                  title="Last re-check"
                  ok={lastRecheck.passed}
                  body={`${lastRecheck.gate.http_status ?? "—"} in ${lastRecheck.gate.response_time_ms ?? "—"}ms · ${lastRecheck.block_reason || "passed"}`}
                />
              )}
              {(mode === "full" || mode === "bing" || mode === "google" || mode === "gsc") && lastSubmit && (
                <LiveBox title={`Last ${lastSubmit.provider} submit`} ok={lastSubmit.success} body={lastSubmit.response_body || lastSubmit.message} />
              )}
              {(mode === "full" || mode === "bing" || mode === "google") && (analysis.recent_submissions?.length ?? 0) > 0 && (
                <div className="mt-3 space-y-2">
                  {analysis.recent_submissions
                    .filter((s) => mode === "full" || s.provider.toLowerCase() === mode)
                    .slice(0, 5)
                    .map((s) => (
                      <div key={s.id} className="p-2 rounded-lg bg-slate-950/50 border border-slate-800 text-[11px]">
                        <div className="flex justify-between">
                          <span className="uppercase text-slate-300">{s.provider}</span>
                          <span className={s.success ? "text-emerald-400" : "text-rose-400"}>
                            {s.success ? "OK" : "Failed"}
                            {s.response_code != null ? ` (${s.response_code})` : ""}
                          </span>
                        </div>
                        {s.response_body && (
                          <pre className="mt-1 text-slate-500 whitespace-pre-wrap break-all max-h-20 overflow-auto">
                            {s.response_body.slice(0, 800)}
                          </pre>
                        )}
                        <div className="text-slate-600 mt-1">{formatDate(s.created_at)}</div>
                      </div>
                    ))}
                </div>
              )}
            </>
          )}
        </div>
      </aside>
    </div>
  );
}

function SignalRow({
  label,
  value,
  extra,
  warn,
}: {
  label: string;
  value: string | null | undefined;
  extra?: string;
  warn?: boolean;
}) {
  return (
    <div className="py-2 border-b border-slate-800/60 last:border-0">
      <div className="flex items-center justify-between gap-2">
        <span className="text-[11px] text-slate-500 font-mono">{label}</span>
        {extra && (
          <span className={`text-[10px] ${warn ? "text-rose-400" : "text-emerald-400"}`}>
            {extra}
          </span>
        )}
      </div>
      <div
        className={`text-sm mt-0.5 break-words ${
          warn ? "text-rose-300" : "text-slate-200"
        }`}
      >
        {value || "—"}
      </div>
    </div>
  );
}

function MiniStat({
  label,
  value,
  ok,
}: {
  label: string;
  value: string;
  ok?: boolean;
}) {
  return (
    <div className="rounded-xl border border-slate-800 bg-slate-950/60 p-3">
      <div className="text-[11px] text-slate-500">{label}</div>
      <div
        className={`text-lg font-semibold mt-0.5 ${
          ok === true ? "text-emerald-400" : ok === false ? "text-rose-400" : "text-white"
        }`}
      >
        {value}
      </div>
    </div>
  );
}

function LiveBox({
  title,
  ok,
  body,
}: {
  title: string;
  ok: boolean;
  body: string;
}) {
  return (
    <div
      className={`mb-2 p-2.5 rounded-xl border text-xs ${
        ok
          ? "border-emerald-800/50 bg-emerald-950/20 text-emerald-200"
          : "border-rose-800/50 bg-rose-950/20 text-rose-200"
      }`}
    >
      <div className="flex items-center gap-1.5 font-medium mb-1">
        <CheckCircle2 className="w-3.5 h-3.5" />
        {title}
      </div>
      <pre className="whitespace-pre-wrap break-all max-h-28 overflow-auto text-[11px] opacity-90">
        {body.slice(0, 1200)}
      </pre>
    </div>
  );
}
