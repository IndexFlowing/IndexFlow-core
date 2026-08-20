"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { ChevronLeft, ChevronRight, Lock, ShieldCheck, Zap } from "lucide-react";
import { api } from "@/lib/api";
import { clearSession, setSession } from "@/lib/auth";

const SLIDES = [
  {
    image: "/carousel/slide-discovery.jpg",
    title: "Sitemap discovery & URL lifecycle",
    desc: "Parse Sitemap / Index files, ingest millions of URLs, and manage discovery, inspection, and submission in one pipeline.",
  },
  {
    image: "/carousel/slide-health.jpg",
    title: "Standalone SEO quality gate",
    desc: "HTTP 200, title, description, canonical, robots, and H1. Run a full audit or unchecked-only scan without submitting.",
  },
  {
    image: "/carousel/slide-pipeline.jpg",
    title: "Four decoupled workspaces",
    desc: "Sitemap assets, SEO inspection, engine push, and GSC index monitoring are independent workflows on one site header.",
  },
  {
    image: "/carousel/slide-channels.jpg",
    title: "Bing / Google push & GSC exemption",
    desc: "IndexNow batches at full speed. Google uses a rolling 24-hour quota. Ranking URLs harvested from GSC skip that quota.",
  },
];

export default function LoginPage() {
  const router = useRouter();
  const [setupRequired, setSetupRequired] = useState(false);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  const [slide, setSlide] = useState(0);

  const boot = useCallback(async () => {
    try {
      const s = await api.authStatus();
      if (s.authenticated) {
        router.replace("/");
        return;
      }
      setSetupRequired(s.setup_required);
      if (s.setup_required) {
        clearSession();
      }
    } catch {
      setSetupRequired(false);
    } finally {
      setLoading(false);
    }
  }, [router]);

  useEffect(() => {
    boot();
  }, [boot]);

  useEffect(() => {
    const t = setInterval(() => {
      setSlide((i) => (i + 1) % SLIDES.length);
    }, 5500);
    return () => clearInterval(t);
  }, []);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (setupRequired) {
      if (password !== password2) {
        setError("Passwords do not match");
        return;
      }
    }
    setSubmitting(true);
    try {
      const res = setupRequired
        ? await api.authSetup(username, password)
        : await api.authLogin(username, password);
      setSession(res.token, res.username);
      router.replace("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Sign-in failed");
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center text-slate-400 text-sm">
        Loading…
      </div>
    );
  }

  const current = SLIDES[slide];

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col lg:flex-row">
      {/* Left: auth */}
      <div className="w-full lg:w-[42%] xl:w-[38%] flex flex-col justify-center px-6 sm:px-10 py-10 border-b lg:border-b-0 lg:border-r border-slate-800">
        <div className="max-w-md w-full mx-auto">
          <div className="flex items-center gap-3 mb-8">
            <div className="p-2.5 rounded-xl bg-indigo-600 shadow-lg shadow-indigo-500/30">
              <Zap className="w-5 h-5 text-white" />
            </div>
            <div>
              <div className="font-semibold text-lg tracking-tight">IndexFlow</div>
              <div className="text-xs text-slate-500">Search Index Infrastructure</div>
            </div>
          </div>

          <h1 className="text-2xl font-semibold text-white mb-2">
            {setupRequired ? "Create admin account" : "Admin sign in"}
          </h1>
          <p className="text-sm text-slate-400 mb-8 leading-relaxed">
            {setupRequired
              ? "Set an admin username and password. After that you can manage sites, sitemaps, and search-engine submission."
              : "Sign in to manage URL lifecycle and search-engine channels."}
          </p>

          <form onSubmit={onSubmit} className="space-y-4">
            <label className="block">
              <span className="text-xs text-slate-400 mb-1 block">Username</span>
              <input
                required
                autoComplete="username"
                minLength={3}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2.5 text-sm text-slate-100"
                placeholder="admin"
              />
            </label>
            <label className="block">
              <span className="text-xs text-slate-400 mb-1 block">Password</span>
              <input
                required
                type="password"
                autoComplete={setupRequired ? "new-password" : "current-password"}
                minLength={6}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2.5 text-sm text-slate-100"
                placeholder="At least 6 characters"
              />
            </label>
            {setupRequired && (
              <label className="block">
                <span className="text-xs text-slate-400 mb-1 block">Confirm password</span>
                <input
                  required
                  type="password"
                  autoComplete="new-password"
                  minLength={6}
                  value={password2}
                  onChange={(e) => setPassword2(e.target.value)}
                  className="w-full bg-slate-900 border border-slate-800 rounded-xl px-3 py-2.5 text-sm text-slate-100"
                />
              </label>
            )}

            {error && (
              <div className="text-sm text-rose-400 bg-rose-950/40 border border-rose-900/50 rounded-xl px-3 py-2">
                {error}
              </div>
            )}

            <button
              type="submit"
              disabled={submitting}
              className="w-full py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {setupRequired ? (
                <ShieldCheck className="w-4 h-4" />
              ) : (
                <Lock className="w-4 h-4" />
              )}
              {submitting
                ? "Working…"
                : setupRequired
                  ? "Create admin and continue"
                  : "Sign in"}
            </button>
          </form>

          <p className="text-[11px] text-slate-600 mt-8 leading-relaxed">
            Community Edition · Data stays in your local PostgreSQL · Keep the admin password safe
          </p>
        </div>
      </div>

      {/* Right: carousel */}
      <div className="flex-1 relative min-h-[42vh] lg:min-h-screen overflow-hidden bg-slate-900">
        {SLIDES.map((s, i) => (
          <div
            key={s.image}
            className={`absolute inset-0 transition-opacity duration-700 ${
              i === slide ? "opacity-100" : "opacity-0"
            }`}
          >
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img
              src={s.image}
              alt={s.title}
              className="absolute inset-0 w-full h-full object-cover"
            />
            <div className="absolute inset-0 bg-gradient-to-t from-slate-950 via-slate-950/55 to-slate-950/20" />
          </div>
        ))}

        <div className="absolute inset-x-0 bottom-0 p-6 sm:p-10 z-10">
          <div className="max-w-xl">
            <div className="text-[11px] uppercase tracking-widest text-indigo-300/80 mb-2">
              Product Tour · {slide + 1}/{SLIDES.length}
            </div>
            <h2 className="text-xl sm:text-2xl font-semibold text-white mb-2">
              {current.title}
            </h2>
            <p className="text-sm text-slate-300 leading-relaxed mb-6">
              {current.desc}
            </p>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() =>
                  setSlide((i) => (i - 1 + SLIDES.length) % SLIDES.length)
                }
                className="p-2 rounded-lg border border-slate-700 bg-slate-950/50 text-slate-300 hover:bg-slate-900"
                aria-label="Previous slide"
              >
                <ChevronLeft className="w-4 h-4" />
              </button>
              <div className="flex gap-1.5">
                {SLIDES.map((_, i) => (
                  <button
                    key={i}
                    type="button"
                    onClick={() => setSlide(i)}
                    className={`h-1.5 rounded-full transition-all ${
                      i === slide
                        ? "w-6 bg-indigo-400"
                        : "w-1.5 bg-slate-600 hover:bg-slate-500"
                    }`}
                    aria-label={`Slide ${i + 1}`}
                  />
                ))}
              </div>
              <button
                type="button"
                onClick={() => setSlide((i) => (i + 1) % SLIDES.length)}
                className="p-2 rounded-lg border border-slate-700 bg-slate-950/50 text-slate-300 hover:bg-slate-900"
                aria-label="Next slide"
              >
                <ChevronRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
