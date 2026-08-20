"use client";

import { ReactNode, useEffect, useState } from "react";
import { api } from "@/lib/api";
import { clearSession, getToken, setSession } from "@/lib/auth";

function goLogin() {
  if (typeof window === "undefined") return;
  if (!window.location.pathname.startsWith("/login")) {
    window.location.replace("/login/");
  }
}

export function AuthGuard({ children }: { children: ReactNode }) {
  const [ready, setReady] = useState(false);
  const [hint, setHint] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await api.authStatus();
        if (cancelled) return;
        if (status.setup_required || !status.authenticated) {
          if (!status.authenticated && getToken()) {
            clearSession();
          }
          goLogin();
          return;
        }
        if (status.username && getToken()) {
          setSession(getToken()!, status.username);
        }
        setReady(true);
      } catch (e) {
        if (cancelled) return;
        const msg = e instanceof Error ? e.message : "Could not reach the API";
        setHint(msg);
        // Hard navigation so a stuck SPA router cannot leave this screen forever.
        goLogin();
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (!ready) {
    return (
      <div className="min-h-screen bg-slate-950 flex flex-col items-center justify-center text-slate-400 text-sm gap-3 px-6">
        <div>{hint ? hint : "Checking session…"}</div>
        {hint && (
          <a href="/login/" className="text-indigo-400 hover:underline">
            Continue to sign in
          </a>
        )}
      </div>
    );
  }

  return <>{children}</>;
}
