"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { ArrowLeft, Loader2 } from "lucide-react";
import { Shell } from "@/components/Shell";
import { SiteNav } from "@/components/SiteNav";
import { ProviderStatusBadge } from "@/components/ProviderStatus";
import { api, SiteDetail } from "@/lib/api";

export default function SiteLayout({ children }: { children: React.ReactNode }) {
  const params = useParams<{ id: string }>();
  const raw = params?.id;
  const id = Number(Array.isArray(raw) ? raw[0] : raw);
  const valid = Number.isFinite(id) && id > 0;

  const [detail, setDetail] = useState<SiteDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!valid) return;
    let cancelled = false;
    api
      .getSite(id)
      .then((d) => {
        if (!cancelled) setDetail(d);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : "Failed to load site");
      });
    return () => {
      cancelled = true;
    };
  }, [id, valid]);

  if (!valid) {
    return (
      <Shell title="Site workbench" subtitle="Invalid site ID">
        <div className="rounded-xl border border-rose-800/50 bg-rose-950/30 p-4 text-sm text-rose-300">
          Invalid site ID.
        </div>
        <Link href="/" className="inline-flex items-center gap-1.5 mt-4 text-sm text-indigo-400 hover:underline">
          <ArrowLeft className="w-4 h-4" />
          Back to overview
        </Link>
      </Shell>
    );
  }

  // While loading site meta, still show Shell with fallback title so nav is visible early
  const title = detail?.site.domain ?? `Site #${id}`;
  const subtitle = detail ? `Site workbench · ${detail.site.domain}` : "Loading site…";

  return (
    <Shell title={title} subtitle={subtitle}>
      {error && (
        <div className="mb-4 p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-sm">
          {error}
        </div>
      )}
      <div className="flex flex-col lg:flex-row gap-6">
        <aside className="lg:w-52 shrink-0">
          <div className="lg:sticky lg:top-6 space-y-4">
            <div className="rounded-2xl border border-slate-800 bg-slate-900/40 overflow-hidden">
              <SiteNav siteId={id} />
              {detail?.site && (
                <div className="px-3 py-3 border-t border-slate-800 space-y-2">
                  <div className="text-[11px] text-slate-500">Providers</div>
                  <div className="flex flex-wrap gap-1.5">
                    <ProviderStatusBadge
                      name="Bing"
                      status={detail.site.indexnow_status}
                      error={detail.site.indexnow_last_error}
                      compact
                    />
                    <ProviderStatusBadge
                      name="Google"
                      status={detail.site.google_status}
                      error={detail.site.google_last_error}
                      compact
                    />
                  </div>
                </div>
              )}
            </div>
            {!detail && !error && (
              <div className="flex items-center gap-2 text-xs text-slate-500 px-2">
                <Loader2 className="w-3.5 h-3.5 animate-spin" />
                Loading site…
              </div>
            )}
          </div>
        </aside>
        <div className="flex-1 min-w-0">{children}</div>
      </div>
    </Shell>
  );
}

// Static export: allow client-side dynamic ids
export function generateStaticParams() {
  return [];
}
