import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(iso: string | null | undefined): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString("en-US", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  } catch {
    return iso;
  }
}

export function formatNumber(n: number): string {
  return new Intl.NumberFormat("en-US").format(n);
}

/** Status badge color classes */
export function statusTone(status: string): string {
  const s = status.toUpperCase();
  if (["READY", "SUCCESS", "SUBMITTED", "ACTIVE"].includes(s)) {
    return "bg-emerald-950/70 text-emerald-400 border-emerald-800/50";
  }
  if (["PENDING", "PROCESSING", "SCANNING"].includes(s)) {
    return "bg-amber-950/70 text-amber-400 border-amber-800/50";
  }
  if (["BLOCKED", "FAILED", "NEED_ATTENTION"].includes(s)) {
    return "bg-rose-950/70 text-rose-400 border-rose-800/50";
  }
  if (["CREATED", "NONE"].includes(s)) {
    return "bg-slate-800 text-slate-300 border-slate-700";
  }
  return "bg-slate-800/80 text-slate-300 border-slate-700";
}

export function statusLabel(status: string): string {
  const map: Record<string, string> = {
    CREATED: "Created",
    SCANNING: "Scanning",
    READY: "Ready",
    NEED_ATTENTION: "Needs attention",
    FAILED: "Failed",
    PENDING: "Pending",
    SUBMITTED: "Submitted",
    BLOCKED: "Blocked",
    NONE: "Not submitted",
    PROCESSING: "Running",
    SUCCESS: "Success",
    SYNC_SITEMAP: "Sync sitemap",
    CHECK_URL: "SEO quality gate (retired)",
    SUBMIT_URL: "Inspect SEO & submit",
    SUBMIT_BING: "Submit to Bing",
    SUBMIT_GOOGLE: "Submit to Google",
    RETRY_SUBMISSION: "Retry submission",
    ACTIVE: "Active",
    RECOVERING: "Recovering",
    INDEX: "Sitemap Index",
    URL_SET: "URL Set",
  };
  return map[status.toUpperCase()] || status;
}

export function httpStatusLabel(code: number | null | undefined): string {
  if (code == null) return "—";
  const map: Record<number, string> = {
    200: "OK",
    201: "Created",
    301: "Moved Permanently",
    302: "Found",
    304: "Not Modified",
    400: "Bad Request",
    401: "Unauthorized",
    403: "Forbidden",
    404: "Not Found",
    410: "Gone",
    429: "Too Many Requests",
    500: "Internal Server Error",
    502: "Bad Gateway",
    503: "Service Unavailable",
  };
  return map[code] || "";
}

export function formatHttpDiag(
  code: number | null | undefined,
  ms: number | null | undefined
): string {
  if (code == null) return "—";
  const label = httpStatusLabel(code);
  const base = label ? `${code} ${label}` : String(code);
  if (ms != null) return `${base} (${ms}ms)`;
  return base;
}
