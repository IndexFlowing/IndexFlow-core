/** Provider credential fill vs verify status badges */

export type ProviderStatusCode = "UNSET" | "SAVED" | "VERIFIED" | "FAILED" | string;

const LABELS: Record<string, string> = {
  UNSET: "Unset",
  SAVED: "Saved",
  VERIFIED: "Verified",
  FAILED: "Failed",
};

const STYLES: Record<string, string> = {
  UNSET: "bg-slate-800/80 text-slate-400 border-slate-700",
  SAVED: "bg-amber-950/70 text-amber-300 border-amber-800/50",
  VERIFIED: "bg-emerald-950/70 text-emerald-400 border-emerald-800/50",
  FAILED: "bg-rose-950/70 text-rose-400 border-rose-800/50",
};

export function providerStatusLabel(status?: string | null): string {
  if (!status) return LABELS.UNSET;
  return LABELS[status] || status;
}

export function ProviderStatusBadge({
  name,
  status,
  error,
  compact,
}: {
  name: string;
  status?: string | null;
  error?: string | null;
  compact?: boolean;
}) {
  const code = (status || "UNSET").toUpperCase();
  const style = STYLES[code] || STYLES.UNSET;
  const label = providerStatusLabel(code);

  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border text-xs ${
        compact ? "px-2 py-0.5" : "px-2.5 py-1"
      } ${style}`}
      title={error || undefined}
    >
      <span className="text-[10px] opacity-80">{name}</span>
      <span className="font-medium">{label}</span>
      {code === "FAILED" && error ? (
        <span className="max-w-[140px] truncate opacity-80" title={error}>
          · {error.slice(0, 40)}
          {error.length > 40 ? "…" : ""}
        </span>
      ) : null}
    </span>
  );
}
