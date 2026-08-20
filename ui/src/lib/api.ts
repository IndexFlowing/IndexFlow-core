import { authHeader, clearSession } from "@/lib/auth";

/** API base: same origin when served by Axum; Next dev proxies /api → :8010 */
const API = "/api/v1";

type RequestInitEx = RequestInit & { timeoutMs?: number };

async function request<T>(path: string, init?: RequestInitEx): Promise<T> {
  const timeoutMs = init?.timeoutMs ?? 20_000;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  let res: Response;
  try {
    res = await fetch(`${API}${path}`, {
      ...init,
      signal: init?.signal ?? ctrl.signal,
      headers: {
        "Content-Type": "application/json",
        ...authHeader(),
        ...(init?.headers || {}),
      },
    });
  } catch (e) {
    const aborted =
      (e instanceof DOMException && e.name === "AbortError") ||
      (e instanceof Error && e.name === "AbortError");
    if (aborted) {
      throw new Error("Request timed out — is the IndexFlow API running?");
    }
    throw e;
  } finally {
    clearTimeout(timer);
  }

  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    if (res.status === 401 && typeof window !== "undefined") {
      if (!path.startsWith("/auth/")) {
        clearSession();
        if (!window.location.pathname.startsWith("/login")) {
          window.location.href = "/login/";
        }
      }
    }
    const msg =
      (body && (body.error || body.message)) || `HTTP ${res.status}`;
    throw new Error(typeof msg === "string" ? msg : JSON.stringify(msg));
  }
  return body as T;
}

// ── Types ──────────────────────────────────────────────

export type ProviderCredentialStatus =
  | "UNSET"
  | "SAVED"
  | "VERIFIED"
  | "FAILED";

export type UrlLifecycleStatus = "PENDING" | "SUBMITTED" | "BLOCKED";

export interface Site {
  id: number;
  domain: string;
  status: string;
  indexnow_key: string | null;
  google_service_account_json: string | null;
  indexnow_status: ProviderCredentialStatus | string;
  indexnow_last_error: string | null;
  indexnow_verified_at: string | null;
  google_status: ProviderCredentialStatus | string;
  google_last_error: string | null;
  google_verified_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface SiteUrlStats {
  site_id: number;
  url_total: number;
  pending: number;
  submitted: number;
  blocked: number;
  bing_submitted_count: number;
  bing_pending_count: number;
  google_submitted_count: number;
  google_pending_count: number;
}

export interface SiteActivity {
  running: boolean;
  phase: string;
  label: string;
  sync_pending: number;
  sync_processing: number;
  submit_pending: number;
  submit_processing: number;
}

export interface SiteDetail extends SiteUrlStats {
  site: Site;
  activity: SiteActivity;
  google_quota_used: number;
  google_quota_total: number;
  google_quota_remaining: number;
  google_quota_next_free_at: string | null;
}

export interface SiteWorkbenchSummary {
  site: Site;
  url_total: number;
  pending: number;
  submitted: number;
  blocked: number;
  bing_submitted_count: number;
  bing_pending_count: number;
  google_submitted_count: number;
  google_pending_count: number;
  activity: SiteActivity;
  google_quota_used: number;
  google_quota_total: number;
  google_quota_remaining: number;
  google_quota_next_free_at: string | null;
}

export interface LocaleCount {
  locale: string;
  count: number;
}

export interface PathPrefixCount {
  path_prefix: string;
  count: number;
}

export interface Sitemap {
  id: number;
  site_id: number;
  url: string;
  type: string;
  status: string;
  last_sync_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface UrlItem {
  id: number;
  site_id: number;
  url: string;
  url_hash: string;
  status: string;
  priority: number;
  locale: string;
  path_prefix: string;
  page_title: string | null;
  canonical_url: string | null;
  block_reason: string | null;
  sitemap_priority: number | null;
  sitemap_lastmod: string | null;
  first_seen_at: string;
  last_seen_at: string;
  last_http_status: number | null;
  last_checked_at: string | null;
  next_check_at: string | null;
  last_submitted_at: string | null;
  bing_status: string;
  google_status: string;
  bing_submitted_at: string | null;
  google_submitted_at: string | null;
  bing_error: string | null;
  google_error: string | null;
  meta_description: string | null;
  h1_content: string | null;
  google_index_status: string;
  google_coverage_state: string | null;
  google_last_crawled_at: string | null;
  google_inspected_at: string | null;
  bing_index_status: string;
  bing_last_crawled_at: string | null;
  bing_inspected_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface UrlDiagnostic {
  id: number;
  site_id: number;
  url: string;
  status: string;
  locale: string;
  path_prefix: string;
  page_title: string | null;
  canonical_url: string | null;
  block_reason: string | null;
  bing_status: string;
  google_status: string;
  bing_submitted_at: string | null;
  google_submitted_at: string | null;
  bing_error: string | null;
  google_error: string | null;
  queue_status: string | null;
  priority: number;
  sitemap_priority: number | null;
  sitemap_lastmod: string | null;
  last_http_status: number | null;
  last_checked_at: string | null;
  last_submitted_at: string | null;
  meta_description: string | null;
  h1_content: string | null;
  google_index_status: string;
  google_coverage_state: string | null;
  google_last_crawled_at: string | null;
  google_inspected_at: string | null;
  bing_index_status: string;
  updated_at: string;
}

export interface HealthCheck {
  id: number;
  url_id: number;
  http_status: number | null;
  response_time: number | null;
  has_noindex: boolean;
  has_canonical: boolean;
  meta_description: string | null;
  h1_content: string | null;
  robots_directive: string | null;
  payload_bytes: number | null;
  hreflang: string | null;
  checked_at: string;
}

export interface HreflangAlt {
  lang: string;
  href: string;
}

export interface UrlSignals {
  title: string | null;
  title_chars: number;
  meta_description: string | null;
  meta_description_chars: number;
  h1: string | null;
  canonical_url: string | null;
  canonical_matches: boolean | null;
  robots: string | null;
  hreflang: HreflangAlt[];
  http_status: number | null;
  response_time_ms: number | null;
  payload_bytes: number | null;
}

export interface UrlGscTrail {
  index_status: string;
  coverage_state: string | null;
  last_crawled_at: string | null;
  inspected_at: string | null;
}

export interface UrlAnalysis {
  url: UrlItem;
  signals: UrlSignals;
  gsc: UrlGscTrail;
  recent_checks: HealthCheck[];
  recent_submissions: SubmissionLog[];
}

export interface RecheckResult {
  url: UrlItem;
  passed: boolean;
  block_reason: string | null;
  gate: {
    http_status: number | null;
    response_time_ms: number | null;
    page_title: string | null;
    meta_description: string | null;
    h1_content: string | null;
    canonical_url: string | null;
    robots_directive: string | null;
    payload_bytes: number | null;
    passed: boolean;
    block_reason: string | null;
  };
}

export interface SubmitNowResult {
  url: UrlItem;
  provider: string;
  success: boolean;
  status_code: number | null;
  response_body: string | null;
  message: string;
  quota_exempt: boolean;
}

export interface SeoStats {
  site_id: number;
  checked: number;
  unchecked: number;
  blocked: number;
  http_status: { http_status: number | null; count: number }[];
  block_reasons: { reason: string; count: number }[];
}

export interface IndexMonitorStats {
  funnel: {
    site_id: number;
    indexed: number;
    crawled_not_indexed: number;
    discovered_not_indexed: number;
    unknown: number;
    inspected_24h: number;
  };
  gsc_inspect_quota_total: number;
  gsc_inspect_used_24h: number;
  gsc_inspect_remaining: number;
  gsc_inspect_pending: number;
  gsc_property_url: string | null;
  gsc_analytics_synced_at: string | null;
}

export interface GscSyncResult {
  success: boolean;
  property_url: string;
  pages_from_gsc: number;
  urls_marked_indexed: number;
  message: string;
}

export interface GscInspectEnqueueResult {
  success: boolean;
  tasks_created: number;
  quota_used_24h: number;
  quota_remaining: number;
  message: string;
}

export interface SubmissionLog {
  id: number;
  url_id: number;
  provider: string;
  success: boolean;
  response_code: number | null;
  response_body: string | null;
  created_at: string;
}

export interface UrlDetail {
  url: UrlItem;
  recent_checks: HealthCheck[];
  recent_submissions: SubmissionLog[];
}

export interface Task {
  id: number;
  site_id: number;
  url_id: number | null;
  sitemap_id: number | null;
  task_type: string;
  status: string;
  priority: number;
  scheduled_at: string;
  started_at: string | null;
  finished_at: string | null;
  retry_count: number;
  locked_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface PageResponse<T> {
  items: T[];
  total: number;
  page: number;
  limit: number;
}

export interface ConfigInfo {
  submit_worker_interval_secs: number;
  scheduler_interval_secs: number;
  worker_poll_interval_secs: number;
  submit_worker_batch: number;
  google_daily_quota: number;
  gsc_inspect_daily_quota?: number;
}

export interface DashboardCounts {
  sites: SiteWorkbenchSummary[];
  config_info: ConfigInfo;
}

export interface CreateSiteInput {
  domain: string;
  sitemap_url?: string | null;
  indexnow_key?: string | null;
  google_service_account_json?: string | null;
}

export interface UpdateSiteCredentialsInput {
  indexnow_key?: string | null;
  google_service_account_json?: string | null;
  set_indexnow_key?: boolean;
  set_google_service_account_json?: boolean;
}

export interface WorkflowResult {
  success: boolean;
  tasks_created: number;
  message: string;
}

export interface ChannelTestResult {
  success: boolean;
  provider: string;
  credential_status: ProviderCredentialStatus | string;
  message: string;
  status_code: number | null;
}

export interface AuthStatus {
  setup_required: boolean;
  authenticated: boolean;
  username: string | null;
}

export interface AuthTokenResponse {
  token: string;
  username: string;
  expires_at: string;
}

export interface UrlListOpts {
  status?: string;
  locale?: string;
  path_prefix?: string;
  page?: number;
  limit?: number;
  seo_checked?: boolean;
  google_index_status?: string;
}

function qs(
  params: Record<string, string | number | boolean | undefined | null>
): string {
  const q = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v != null && v !== "") q.set(k, String(v));
  }
  const s = q.toString();
  return s ? `?${s}` : "";
}

// ── API ────────────────────────────────────────────────

export const api = {
  health: () => request<{ status: string }>("/health"),

  authStatus: () => request<AuthStatus>("/auth/status", { timeoutMs: 8000 }),

  authSetup: (username: string, password: string) =>
    request<AuthTokenResponse>("/auth/setup", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    }),

  authLogin: (username: string, password: string) =>
    request<AuthTokenResponse>("/auth/login", {
      method: "POST",
      body: JSON.stringify({ username, password }),
    }),

  authMe: () => request<{ id: number; username: string }>("/auth/me"),

  dashboard: () => request<DashboardCounts>("/dashboard"),

  listSites: () =>
    request<{ sites: Site[] }>("/sites").then((r) => r.sites),

  getSite: (id: number) => request<SiteDetail>(`/sites/${id}`),

  getSiteStats: (
    siteId: number,
    opts?: { locale?: string; path_prefix?: string }
  ) =>
    request<SiteUrlStats>(
      `/sites/${siteId}/stats${qs({
        locale: opts?.locale,
        path_prefix: opts?.path_prefix,
      })}`
    ),

  listLocales: (siteId: number, pathPrefix?: string) =>
    request<{ locales: LocaleCount[] }>(
      `/sites/${siteId}/locales${qs({ path_prefix: pathPrefix })}`
    ).then((r) => r.locales),

  listPathPrefixes: (siteId: number, locale?: string) =>
    request<{ path_prefixes: PathPrefixCount[] }>(
      `/sites/${siteId}/path-prefixes${qs({ locale })}`
    ).then((r) => r.path_prefixes),

  createSite: (input: CreateSiteInput) =>
    request<{ site: Site }>("/sites", {
      method: "POST",
      body: JSON.stringify(input),
    }).then((r) => r.site),

  updateSiteCredentials: (id: number, input: UpdateSiteCredentialsInput) =>
    request<{ success: boolean; site: Site; message: string }>(`/sites/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),

  listSitemaps: (siteId: number) =>
    request<{ sitemaps: Sitemap[] }>(`/sites/${siteId}/sitemaps`).then(
      (r) => r.sitemaps
    ),

  syncSitemap: (siteId: number, sitemapUrl?: string) =>
    request<WorkflowResult>(`/sites/${siteId}/sitemap/sync`, {
      method: "POST",
      body: JSON.stringify({ sitemap_url: sitemapUrl || null }),
    }),

  startSubmit: (siteId: number) =>
    request<WorkflowResult>(`/sites/${siteId}/submit`, {
      method: "POST",
    }),

  startSubmitBing: (siteId: number) =>
    request<WorkflowResult>(`/sites/${siteId}/submit-bing`, {
      method: "POST",
    }),

  startSubmitGoogle: (siteId: number) =>
    request<WorkflowResult>(`/sites/${siteId}/submit-google`, {
      method: "POST",
    }),

  seoAuditFull: (siteId: number) =>
    request<WorkflowResult>(`/sites/${siteId}/seo/audit`, {
      method: "POST",
    }),

  seoAuditUnchecked: (siteId: number) =>
    request<WorkflowResult>(`/sites/${siteId}/seo/audit-unchecked`, {
      method: "POST",
    }),

  getSeoStats: (siteId: number) => request<SeoStats>(`/sites/${siteId}/seo-stats`),

  getIndexStats: (siteId: number) =>
    request<IndexMonitorStats>(`/sites/${siteId}/index-stats`),

  gscSyncAnalytics: (siteId: number) =>
    request<GscSyncResult>(`/sites/${siteId}/gsc/sync-analytics`, {
      method: "POST",
      timeoutMs: 180_000,
    }),

  gscInspectBatch: (siteId: number) =>
    request<GscInspectEnqueueResult>(`/sites/${siteId}/gsc/inspect-batch`, {
      method: "POST",
    }),

  testBing: (siteId: number) =>
    request<ChannelTestResult>(`/sites/${siteId}/test-bing`, {
      method: "POST",
    }),

  testGoogle: (siteId: number) =>
    request<ChannelTestResult>(`/sites/${siteId}/test-google`, {
      method: "POST",
    }),

  listUrls: (siteId: number, opts?: UrlListOpts) =>
    request<PageResponse<UrlItem>>(
      `/sites/${siteId}/urls${qs({
        status: opts?.status,
        locale: opts?.locale,
        path_prefix: opts?.path_prefix,
        page: opts?.page,
        limit: opts?.limit,
      })}`
    ),

  listDiagnostics: (siteId: number, opts?: UrlListOpts) =>
    request<PageResponse<UrlDiagnostic>>(
      `/sites/${siteId}/url-diagnostics${qs({
        status: opts?.status,
        locale: opts?.locale,
        path_prefix: opts?.path_prefix,
        page: opts?.page,
        limit: opts?.limit,
        seo_checked: opts?.seo_checked,
        google_index_status: opts?.google_index_status,
      })}`
    ),

  getUrl: (id: number) => request<UrlDetail>(`/urls/${id}`),

  getUrlAnalysis: (id: number) => request<UrlAnalysis>(`/urls/${id}/analysis`),

  recheckUrl: (id: number) =>
    request<RecheckResult>(`/urls/${id}/recheck`, {
      method: "POST",
      timeoutMs: 30_000,
    }),

  submitUrlNow: (id: number, provider: "bing" | "google") =>
    request<SubmitNowResult>(`/urls/${id}/submit-now`, {
      method: "POST",
      body: JSON.stringify({ provider }),
    }),

  listTasks: (opts?: { status?: string; page?: number; limit?: number }) =>
    request<PageResponse<Task>>(
      `/tasks${qs({
        status: opts?.status,
        page: opts?.page,
        limit: opts?.limit,
      })}`
    ),

  retryTask: (id: number) =>
    request<{ success: boolean; task: Task }>(`/tasks/${id}/retry`, {
      method: "POST",
    }),
};
