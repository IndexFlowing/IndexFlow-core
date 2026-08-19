"use client";

import { Suspense, useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { ArrowLeft, RefreshCw } from "lucide-react";
import { Shell } from "@/components/Shell";
import { StatusBadge } from "@/components/StatusBadge";
import { api, UrlDetail } from "@/lib/api";
import { formatDate } from "@/lib/utils";

function UrlDetailInner() {
  const params = useSearchParams();
  const id = Number(params.get("id") || 0);
  const [detail, setDetail] = useState<UrlDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    if (!id) return;
    try {
      setError(null);
      setDetail(await api.getUrl(id));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  if (!id) {
    return (
      <Shell title="URL details">
        <p className="text-rose-400 text-sm">Invalid ID</p>
      </Shell>
    );
  }

  const u = detail?.url;

  return (
    <Shell
      title="URL details"
      subtitle={u?.url}
      actions={
        <>
          {u && (
            <Link
              href={`/site/?id=${u.site_id}`}
              className="flex items-center gap-1.5 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-800 text-slate-300"
            >
              <ArrowLeft className="w-4 h-4" />
              Back to site
            </Link>
          )}
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
        </>
      }
    >
      {error && (
        <div className="mb-4 p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-sm">
          {error}
        </div>
      )}

      {u && (
        <div className="space-y-6">
          <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
            <div className="flex flex-wrap items-center gap-2 mb-3">
              <StatusBadge status={u.status} />
              <span className="text-xs font-mono text-slate-400">
                Bing {u.bing_status || "NONE"}
              </span>
              <span className="text-xs font-mono text-slate-400">
                Google {u.google_status || "NONE"}
              </span>
              <span className="text-xs font-mono text-slate-400">
                {u.locale} · {u.path_prefix}
              </span>
              {u.last_http_status != null && (
                <span className="text-xs font-mono text-slate-400">
                  HTTP {u.last_http_status}
                </span>
              )}
            </div>
            {u.page_title && (
              <div className="text-sm text-slate-200 mb-2">{u.page_title}</div>
            )}
            {u.block_reason && (
              <div className="text-xs text-rose-400 mb-2">Blocked: {u.block_reason}</div>
            )}
            {u.canonical_url && (
              <div className="text-[11px] text-slate-500 mb-2 break-all">
                Canonical：{u.canonical_url}
              </div>
            )}
            <a
              href={u.url}
              target="_blank"
              rel="noreferrer"
              className="text-indigo-300 break-all text-sm hover:underline font-mono"
            >
              {u.url}
            </a>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mt-4 text-xs text-slate-400">
              <div>
                <div className="text-slate-500">First seen</div>
                <div className="mt-0.5">{formatDate(u.first_seen_at)}</div>
              </div>
              <div>
                <div className="text-slate-500">Last seen</div>
                <div className="mt-0.5">{formatDate(u.last_seen_at)}</div>
              </div>
              <div>
                <div className="text-slate-500">Last inspected</div>
                <div className="mt-0.5">{formatDate(u.last_checked_at)}</div>
              </div>
              <div>
                <div className="text-slate-500">Last submitted</div>
                <div className="mt-0.5">{formatDate(u.last_submitted_at)}</div>
              </div>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <h2 className="font-semibold text-white mb-3">Quality gate history</h2>
              {(detail?.recent_checks?.length ?? 0) === 0 ? (
                <p className="text-sm text-slate-500">No records yet</p>
              ) : (
                <div className="space-y-2">
                  {detail!.recent_checks.map((c) => (
                    <div
                      key={c.id}
                      className="p-3 rounded-xl bg-slate-950/50 border border-slate-800 text-xs"
                    >
                      <div className="flex justify-between text-slate-400">
                        <span>HTTP {c.http_status ?? "—"}</span>
                        <span>{c.response_time ?? "—"} ms</span>
                      </div>
                      <div className="mt-1 text-slate-500">
                        noindex={String(c.has_noindex)} · canonical=
                        {String(c.has_canonical)}
                      </div>
                      <div className="mt-1 text-slate-600">{formatDate(c.checked_at)}</div>
                    </div>
                  ))}
                </div>
              )}
            </section>

            <section className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5">
              <h2 className="font-semibold text-white mb-3">Submission log</h2>
              {(detail?.recent_submissions?.length ?? 0) === 0 ? (
                <p className="text-sm text-slate-500">No records yet</p>
              ) : (
                <div className="space-y-2">
                  {detail!.recent_submissions.map((s) => (
                    <div
                      key={s.id}
                      className="p-3 rounded-xl bg-slate-950/50 border border-slate-800 text-xs"
                    >
                      <div className="flex justify-between">
                        <span className="text-slate-300 uppercase">{s.provider}</span>
                        <span className={s.success ? "text-emerald-400" : "text-rose-400"}>
                          {s.success ? "Success" : "Failed"}
                          {s.response_code != null ? ` (${s.response_code})` : ""}
                        </span>
                      </div>
                      {s.response_body && (
                        <pre className="mt-1 text-slate-500 whitespace-pre-wrap break-all max-h-24 overflow-auto">
                          {s.response_body.slice(0, 500)}
                        </pre>
                      )}
                      <div className="mt-1 text-slate-600">{formatDate(s.created_at)}</div>
                    </div>
                  ))}
                </div>
              )}
            </section>
          </div>
        </div>
      )}
    </Shell>
  );
}

export default function UrlPage() {
  return (
    <Suspense
      fallback={
        <Shell title="URL details">
          <div className="text-slate-500 text-sm">Loading…</div>
        </Shell>
      }
    >
      <UrlDetailInner />
    </Suspense>
  );
}
