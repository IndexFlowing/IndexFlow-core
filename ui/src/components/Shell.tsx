"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  Activity,
  Globe,
  LayoutDashboard,
  ListTodo,
  LogOut,
  Zap,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { ReactNode, useEffect, useState } from "react";
import { AuthGuard } from "@/components/AuthGuard";
import { SiteSwitcher } from "@/components/SiteSwitcher";
import { clearSession, getUsername } from "@/lib/auth";
import { api, SiteWorkbenchSummary } from "@/lib/api";

const nav = [
  { href: "/", label: "Overview", icon: LayoutDashboard },
  { href: "/sites", label: "Sites", icon: Globe },
  { href: "/tasks", label: "Tasks", icon: ListTodo },
];

export function Shell({
  children,
  title,
  subtitle,
  actions,
  hideSiteSwitcher,
}: {
  children: ReactNode;
  title: string;
  subtitle?: string;
  actions?: ReactNode;
  hideSiteSwitcher?: boolean;
}) {
  const pathname = usePathname();
  const router = useRouter();
  const username = getUsername();
  const [sites, setSites] = useState<SiteWorkbenchSummary[]>([]);
  const selectedId = (() => {
    const m = pathname.match(/^\/site\/(\d+)/);
    return m ? Number(m[1]) : null;
  })();

  useEffect(() => {
    if (hideSiteSwitcher) return;
    api
      .dashboard()
      .then((d) => setSites(d.sites ?? []))
      .catch(() => {});
  }, [hideSiteSwitcher, pathname]);

  const logout = () => {
    clearSession();
    router.replace("/login/");
  };

  return (
    <AuthGuard>
    <div className="min-h-screen bg-slate-950 text-slate-100">
      <div className="flex min-h-screen">
        {/* Sidebar */}
        <aside className="hidden md:flex w-56 flex-col border-r border-slate-800/80 bg-slate-950/90">
          <div className="px-5 py-6 border-b border-slate-800/80">
            <div className="flex items-center gap-2.5">
              <div className="p-2 rounded-xl bg-indigo-600 shadow-lg shadow-indigo-500/25">
                <Zap className="w-4 h-4 text-white" />
              </div>
              <div>
                <div className="font-semibold text-sm tracking-tight">IndexFlow</div>
                <div className="text-[11px] text-slate-500">Search Index Infra</div>
              </div>
            </div>
          </div>
          {!hideSiteSwitcher && <SiteSwitcher sites={sites} selectedId={selectedId} />}
          <nav className="flex-1 p-3 space-y-1">
            {nav.map((item) => {
              const active =
                item.href === "/"
                  ? pathname === "/"
                  : pathname.startsWith(item.href);
              const Icon = item.icon;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={cn(
                    "flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition",
                    active
                      ? "bg-indigo-600/15 text-indigo-300 border border-indigo-500/20"
                      : "text-slate-400 hover:text-slate-200 hover:bg-slate-900 border border-transparent"
                  )}
                >
                  <Icon className="w-4 h-4" />
                  {item.label}
                </Link>
              );
            })}
          </nav>
          <div className="p-4 border-t border-slate-800/80 space-y-2">
            <div className="flex items-center gap-2 text-xs text-emerald-400">
              <Activity className="w-3.5 h-3.5" />
              Core Engine
            </div>
            {username && (
              <div className="text-[11px] text-slate-500 truncate" title={username}>
                {username}
              </div>
            )}
            <button
              type="button"
              onClick={logout}
              className="flex items-center gap-1.5 text-xs text-slate-400 hover:text-rose-300 transition"
            >
              <LogOut className="w-3.5 h-3.5" />
              Log out
            </button>
          </div>
        </aside>

        {/* Main */}
        <div className="flex-1 flex flex-col min-w-0">
          {/* Mobile top nav */}
          <div className="md:hidden flex items-center gap-1 px-3 py-2 border-b border-slate-800 overflow-x-auto">
            {nav.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={cn(
                  "px-3 py-1.5 rounded-lg text-xs whitespace-nowrap",
                  pathname === item.href ||
                    (item.href !== "/" && pathname.startsWith(item.href))
                    ? "bg-indigo-600/20 text-indigo-300"
                    : "text-slate-400"
                )}
              >
                {item.label}
              </Link>
            ))}
          </div>

          <header className="px-5 md:px-8 py-6 border-b border-slate-800/60 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
            <div>
              <h1 className="text-xl md:text-2xl font-semibold tracking-tight text-white">
                {title}
              </h1>
              {subtitle && (
                <p className="text-sm text-slate-400 mt-1">{subtitle}</p>
              )}
            </div>
            {actions && <div className="flex items-center gap-2">{actions}</div>}
          </header>

          <main className="flex-1 px-5 md:px-8 py-6">{children}</main>
        </div>
      </div>
    </div>
    </AuthGuard>
  );
}
