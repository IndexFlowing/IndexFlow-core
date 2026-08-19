import { ReactNode } from "react";

export function StatCard({
  label,
  value,
  hint,
  icon,
  accent,
}: {
  label: string;
  value: string | number;
  hint?: string;
  icon?: ReactNode;
  accent?: string;
}) {
  return (
    <div className="p-5 rounded-2xl bg-slate-900/50 border border-slate-800">
      <div className="flex items-center justify-between text-slate-400 mb-2">
        <span className="text-sm font-medium">{label}</span>
        {icon}
      </div>
      <div className={`text-3xl font-bold tracking-tight ${accent || "text-white"}`}>
        {value}
      </div>
      {hint && <p className="text-xs text-slate-500 mt-2">{hint}</p>}
    </div>
  );
}
