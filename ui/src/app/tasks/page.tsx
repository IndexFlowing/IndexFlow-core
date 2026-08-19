"use client";

import { useCallback, useEffect, useState } from "react";
import { RefreshCw, RotateCcw } from "lucide-react";
import { Shell } from "@/components/Shell";
import { StatusBadge } from "@/components/StatusBadge";
import { api, Task } from "@/lib/api";
import { formatDate, formatNumber } from "@/lib/utils";

export default function TasksPage() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [total, setTotal] = useState(0);
  const [status, setStatus] = useState("");
  const [page, setPage] = useState(1);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [retrying, setRetrying] = useState<number | null>(null);
  const limit = 30;

  const load = useCallback(async () => {
    try {
      setError(null);
      const res = await api.listTasks({
        status: status || undefined,
        page,
        limit,
      });
      setTasks(res.items);
      setTotal(res.total);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, [status, page]);

  useEffect(() => {
    setLoading(true);
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [load]);

  const onRetry = async (id: number) => {
    setRetrying(id);
    try {
      await api.retryTask(id);
      await load();
    } catch (e) {
      alert(e instanceof Error ? e.message : "Retry failed");
    } finally {
      setRetrying(null);
    }
  };

  const totalPages = Math.max(1, Math.ceil(total / limit));

  return (
    <Shell
      title="Task monitor"
      subtitle="Created by the scheduler · executed by workers · retry by hand"
      actions={
        <button
          onClick={() => {
            setLoading(true);
            load();
          }}
          className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-slate-900 border border-slate-800 text-slate-300"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
          Refresh
        </button>
      }
    >
      {error && (
        <div className="mb-4 p-3 rounded-xl border border-rose-800/50 bg-rose-950/30 text-rose-300 text-sm">
          {error}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-3 mb-4">
        <select
          value={status}
          onChange={(e) => {
            setPage(1);
            setStatus(e.target.value);
          }}
          className="bg-slate-950 border border-slate-800 rounded-lg px-3 py-1.5 text-sm text-slate-200"
        >
          <option value="">All statuses</option>
          <option value="PENDING">PENDING</option>
          <option value="PROCESSING">PROCESSING</option>
          <option value="SUCCESS">SUCCESS</option>
          <option value="FAILED">FAILED</option>
        </select>
        <span className="text-xs text-slate-500">
          {formatNumber(total)} tasks · page {page}/{totalPages}
        </span>
      </div>

      <div className="rounded-2xl border border-slate-800 bg-slate-900/40 overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-xs text-slate-500 border-b border-slate-800 bg-slate-950/40">
                <th className="px-4 py-3 font-medium">ID</th>
                <th className="px-4 py-3 font-medium">Type</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Site</th>
                <th className="px-4 py-3 font-medium">Retries</th>
                <th className="px-4 py-3 font-medium">Error</th>
                <th className="px-4 py-3 font-medium">Created</th>
                <th className="px-4 py-3 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {tasks.length === 0 ? (
                <tr>
                  <td colSpan={8} className="px-4 py-12 text-center text-slate-500">
                    No tasks yet
                  </td>
                </tr>
              ) : (
                tasks.map((t) => (
                  <tr
                    key={t.id}
                    className="border-b border-slate-800/50 last:border-0 hover:bg-slate-900/60"
                  >
                    <td className="px-4 py-2.5 font-mono text-xs text-slate-400">
                      {t.id}
                    </td>
                    <td className="px-4 py-2.5">
                      <StatusBadge status={t.task_type} />
                    </td>
                    <td className="px-4 py-2.5">
                      <StatusBadge status={t.status} />
                    </td>
                    <td className="px-4 py-2.5 text-xs text-slate-400">{t.site_id}</td>
                    <td className="px-4 py-2.5 text-xs text-slate-400">
                      {t.retry_count}
                    </td>
                    <td className="px-4 py-2.5 text-xs text-rose-400/90 max-w-[200px] truncate" title={t.last_error || ""}>
                      {t.last_error || "—"}
                    </td>
                    <td className="px-4 py-2.5 text-xs text-slate-500 whitespace-nowrap">
                      {formatDate(t.created_at)}
                    </td>
                    <td className="px-4 py-2.5">
                      {(t.status === "FAILED" || t.status === "SUCCESS") && (
                        <button
                          onClick={() => onRetry(t.id)}
                          disabled={retrying === t.id}
                          className="inline-flex items-center gap-1 text-xs text-indigo-400 hover:text-indigo-300 disabled:opacity-50"
                        >
                          <RotateCcw
                            className={`w-3.5 h-3.5 ${retrying === t.id ? "animate-spin" : ""}`}
                          />
                          Retry
                        </button>
                      )}
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      <div className="flex items-center justify-end gap-2 mt-4">
        <button
          disabled={page <= 1}
          onClick={() => setPage((p) => Math.max(1, p - 1))}
          className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40"
        >
          Previous
        </button>
        <button
          disabled={page >= totalPages}
          onClick={() => setPage((p) => p + 1)}
          className="px-3 py-1.5 rounded-lg text-sm border border-slate-800 bg-slate-900 text-slate-300 disabled:opacity-40"
        >
          Next
        </button>
      </div>
    </Shell>
  );
}
