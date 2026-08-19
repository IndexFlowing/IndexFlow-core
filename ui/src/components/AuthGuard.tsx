"use client";

import { ReactNode, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { clearSession, getToken, setSession } from "@/lib/auth";

export function AuthGuard({ children }: { children: ReactNode }) {
  const router = useRouter();
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await api.authStatus();
        if (cancelled) return;
        if (status.setup_required || !status.authenticated) {
          // If we have a token that server rejects, clear it
          if (!status.authenticated && getToken()) {
            clearSession();
          }
          router.replace("/login/");
          return;
        }
        if (status.username && getToken()) {
          setSession(getToken()!, status.username);
        }
        setReady(true);
      } catch {
        if (!cancelled) {
          router.replace("/login/");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [router]);

  if (!ready) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center text-slate-400 text-sm">
        Checking session…
      </div>
    );
  }

  return <>{children}</>;
}
