"use client";

import { FormEvent, ReactNode, useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { Plus, RefreshCw, Globe } from "lucide-react";
import { Shell } from "@/components/Shell";
import { StatusBadge } from "@/components/StatusBadge";
import { api, Site } from "@/lib/api";
import { formatDate } from "@/lib/utils";
import { ProviderStatusBadge } from "@/components/ProviderStatus";

export default function SitesPage() {
  const [sites, setSites] = useState<Site[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [domain, setDomain] = useState("");
  const [sitemapUrl, setSitemapUrl] = useState("");
  const [indexnowKey, setIndexnowKey] = useState("");
  const [googleJson, setGoogleJson] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [formMsg, setFormMsg] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [actionMsg, setActionMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setError(null);
      setSites(await api.listSites());
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const onCreate = async (e: FormEvent) => {
    e.preventDefault();
    if (!domain.trim()) return;
    setSubmitting(true);
    setFormMsg(null);
    try {
      const site = await api.createSite({
        domain: domain.trim(),
        sitemap_url: sitemapUrl.trim() || null,
        indexnow_key: indexnowKey.trim() || null,
        google_service_account_json: googleJson.trim() || null,
      });
      setDomain("");
      setSitemapUrl("");
      setIndexnowKey("");
      setGoogleJson("");
      setFormMsg(
        `Added ${site.domain}` +
          (sitemapUrl.trim() ? ". Sitemap sync queued — submit from the site workbench." : "")
      );
      await load();
    } catch (err) {
      setFormMsg(err instanceof Error ? err.message : "Create failed");
    } finally {
      setSubmitting(false);
    }
  };

  const runAction = async (
    key: string,
    fn: () => Promise<{ message?: string }>
  ) => {
    setBusyId(key);
    setActionMsg(null);
    try {
      const res = await fn();
      setActionMsg(res.message || "Done");
      await load();
    } catch (e) {
      setActionMsg(e instanceof Error ? e.message : "Failed");
    } finally {
      setBusyId(null);
    }
  };

  return (
    <Shell
      title="Sites"
      subtitle="Add a site · Open the per-site workbench"
      actions={
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
      }
    >
      {error && (
        <div className="mb-4 p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-sm">
          {error}
        </div>
      )}
      {actionMsg && (
        <div className="mb-4 p-3 rounded-xl border border-slate-700 bg-slate-900 text-slate-300 text-sm">
          {actionMsg}
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2 space-y-3">
          {loading && sites.length === 0 ? (
            <div className="text-slate-500 text-sm py-12 text-center">Loading…</div>
          ) : sites.length === 0 ? (
            <div className="rounded-2xl border border-dashed border-slate-800 py-16 text-center text-slate-500 text-sm">
              No sites yet. Add one on the right.
            </div>
          ) : (
            sites.map((s) => (
              <div
                key={s.id}
                className="p-5 rounded-2xl bg-slate-900/50 border border-slate-800"
              >
                <div className="flex items-start justify-between gap-3 mb-3">
                  <div className="flex items-start gap-3 min-w-0">
                    <div className="p-2 rounded-xl bg-indigo-600/15 border border-indigo-500/20 shrink-0">
                      <Globe className="w-4 h-4 text-indigo-400" />
                    </div>
                    <div className="min-w-0">
                      <Link
                        href={`/site/?id=${s.id}`}
                        className="font-semibold text-white hover:text-indigo-300 truncate block"
                      >
                        {s.domain}
                      </Link>
                      <div className="text-xs text-slate-500 mt-1">
                        ID {s.id} · {formatDate(s.created_at)}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-1.5">
                        <ProviderStatusBadge
                          name="Bing"
                          status={s.indexnow_status}
                          error={s.indexnow_last_error}
                        />
                        <ProviderStatusBadge
                          name="Google"
                          status={s.google_status}
                          error={s.google_last_error}
                        />
                      </div>
                    </div>
                  </div>
                  <StatusBadge status={s.status} />
                </div>

                <div className="flex flex-wrap gap-1.5">
                  <Btn
                    label="Sync sitemap"
                    busy={busyId === `sync-${s.id}`}
                    onClick={() =>
                      runAction(`sync-${s.id}`, () => api.syncSitemap(s.id))
                    }
                  />
                  <Link
                    href={`/site/?id=${s.id}`}
                    className="px-2.5 py-1.5 rounded-lg text-xs border border-indigo-500/40 text-indigo-300 hover:bg-indigo-600/20"
                  >
                    Open workbench →
                  </Link>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="rounded-2xl border border-slate-800 bg-slate-900/40 p-5 h-fit sticky top-4">
          <h2 className="font-semibold text-white mb-4 flex items-center gap-2">
            <Plus className="w-4 h-4 text-indigo-400" />
            Add site
          </h2>
          <form onSubmit={onCreate} className="space-y-3">
            <Field label="Domain *">
              <input
                required
                value={domain}
                onChange={(e) => setDomain(e.target.value)}
                placeholder="example.com"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-sm text-slate-200"
              />
            </Field>
            <Field label="Sitemap URL">
              <input
                type="url"
                value={sitemapUrl}
                onChange={(e) => setSitemapUrl(e.target.value)}
                placeholder="https://example.com/sitemap.xml"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-sm text-slate-200"
              />
            </Field>
            <Field label="Bing IndexNow Key">
              <input
                value={indexnowKey}
                onChange={(e) => setIndexnowKey(e.target.value)}
                placeholder="Optional"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-sm text-slate-200 font-mono text-xs"
              />
            </Field>
            <Field label="Google Service Account JSON">
              <textarea
                rows={5}
                value={googleJson}
                onChange={(e) => setGoogleJson(e.target.value)}
                placeholder="Paste the full JSON, including private_key / client_email …"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 font-mono"
              />
            </Field>
            <p className="text-[11px] text-slate-500 leading-relaxed">
              Adding a site only syncs the sitemap. Run the SEO quality gate and submit from the site workbench.
            </p>
            {formMsg && (
              <div className="text-xs p-2.5 rounded-lg bg-slate-950 border border-slate-800 text-slate-300">
                {formMsg}
              </div>
            )}
            <button
              type="submit"
              disabled={submitting}
              className="w-full py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium disabled:opacity-50 flex items-center justify-center gap-2"
            >
              <Plus className="w-4 h-4" />
              {submitting ? "Saving…" : "Add site"}
            </button>
          </form>
        </div>
      </div>
    </Shell>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="block text-xs text-slate-400 mb-1">{label}</span>
      {children}
    </label>
  );
}

function Btn({
  label,
  onClick,
  busy,
  muted,
}: {
  label: string;
  onClick: () => void;
  busy?: boolean;
  muted?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={busy}
      className={`px-2.5 py-1.5 rounded-lg text-xs border transition disabled:opacity-50 ${
        muted
          ? "bg-slate-900 border-slate-700 text-slate-300 hover:bg-slate-800"
          : "bg-indigo-600/20 border-indigo-500/30 text-indigo-300 hover:bg-indigo-600/30"
      }`}
    >
      {busy ? "…" : label}
    </button>
  );
}
