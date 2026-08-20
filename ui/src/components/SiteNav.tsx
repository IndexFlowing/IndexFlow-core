"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  FileSearch,
  LayoutDashboard,
  Radar,
  ScanSearch,
  Send,
} from "lucide-react";
import { cn } from "@/lib/utils";

const NAV = [
  { href: "dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "sitemaps", label: "Sitemap Assets", icon: FileSearch },
  { href: "seo", label: "SEO Gate", icon: ScanSearch },
  { href: "submissions", label: "Engine Push", icon: Send },
  { href: "monitoring", label: "Index Monitor", icon: Radar },
] as const;

export function SiteNav({ siteId }: { siteId: number }) {
  const pathname = usePathname();
  const base = `/site/${siteId}`;
  const activeLeaf = pathname.split("/").pop() || "dashboard";

  return (
    <nav className="p-2 space-y-1">
      {NAV.map((item) => {
        const active = activeLeaf === item.href;
        const Icon = item.icon;
        return (
          <Link
            key={item.href}
            href={`${base}/${item.href}`}
            className={cn(
              "flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition border",
              active
                ? "bg-indigo-600 text-white border-indigo-500"
                : "text-slate-400 hover:text-slate-100 hover:bg-slate-900 border-transparent"
            )}
          >
            <Icon className="w-4 h-4" />
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
