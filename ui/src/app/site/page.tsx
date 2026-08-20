"use client";

import { Suspense, useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import { Shell } from "@/components/Shell";

function LegacySiteRedirectInner() {
  const params = useSearchParams();
  const router = useRouter();
  const id = params.get("id");
  const tab = params.get("tab");

  useEffect(() => {
    if (id && Number(id) > 0) {
      const map: Record<string, string> = {
        assets: "sitemaps",
        seo: "seo",
        submit: "submissions",
        index: "monitoring",
      };
      const leaf = tab ? map[tab] ?? "dashboard" : "dashboard";
      router.replace(`/site/${id}/${leaf}`);
    }
  }, [id, tab, router]);

  if (!id || Number(id) <= 0) {
    return (
      <Shell title="Site workbench" subtitle="Select a site from the switcher">
        <div className="rounded-2xl border border-dashed border-slate-800 bg-slate-900/30 p-8 text-center">
          <p className="text-sm text-slate-400">No site selected.</p>
          <Link href="/" className="text-sm text-indigo-400 hover:underline mt-2 inline-block">
            Back to overview
          </Link>
        </div>
      </Shell>
    );
  }

  return (
    <Shell title={`Site #${id}`} subtitle="Redirecting…">
      <p className="text-sm text-slate-500">Redirecting to the new workbench…</p>
    </Shell>
  );
}

export default function LegacySitePage() {
  return (
    <Suspense fallback={<Shell title="Site workbench"><p className="text-sm text-slate-500">Loading…</p></Shell>}>
      <LegacySiteRedirectInner />
    </Suspense>
  );
}
