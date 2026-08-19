-- IndexFlow Core schema v1.0
-- Aligns with docs/IndexFlow database design v1.0 (+ provider credentials on sites)

-- 1. sites
CREATE TABLE IF NOT EXISTS sites (
    id              BIGSERIAL PRIMARY KEY,
    domain          VARCHAR(255) NOT NULL UNIQUE,
    status          VARCHAR(50)  NOT NULL DEFAULT 'CREATED',
    -- Community Edition: store provider credentials per site
    indexnow_key                VARCHAR(255),
    google_service_account_json TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sites_status ON sites(status);

-- 2. sitemaps
CREATE TABLE IF NOT EXISTS sitemaps (
    id              BIGSERIAL PRIMARY KEY,
    site_id         BIGINT       NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url             TEXT         NOT NULL,
    type            VARCHAR(50)  NOT NULL DEFAULT 'URL_SET', -- INDEX | URL_SET
    status          VARCHAR(50)  NOT NULL DEFAULT 'ACTIVE',  -- ACTIVE | FAILED | RECOVERING
    last_sync_at    TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, url)
);

CREATE INDEX IF NOT EXISTS idx_sitemaps_site_id ON sitemaps(site_id);

-- 3. urls (core resource table)
CREATE TABLE IF NOT EXISTS urls (
    id                  BIGSERIAL PRIMARY KEY,
    site_id             BIGINT       NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url                 TEXT         NOT NULL,
    url_hash            VARCHAR(64)  NOT NULL,
    status              VARCHAR(50)  NOT NULL DEFAULT 'DISCOVERED',
    first_seen_at       TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    last_seen_at        TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    last_http_status    INTEGER,
    last_checked_at     TIMESTAMPTZ,
    next_check_at       TIMESTAMPTZ,
    last_submitted_at   TIMESTAMPTZ,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    UNIQUE (site_id, url_hash)
);

CREATE INDEX IF NOT EXISTS idx_urls_site_next_check ON urls(site_id, next_check_at);
CREATE INDEX IF NOT EXISTS idx_urls_site_status ON urls(site_id, status);
CREATE INDEX IF NOT EXISTS idx_urls_next_check ON urls(next_check_at) WHERE next_check_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_urls_status_ready ON urls(status) WHERE status = 'READY_SUBMIT';

-- 4. tasks (system actions; not the same as URL)
CREATE TABLE IF NOT EXISTS tasks (
    id              BIGSERIAL PRIMARY KEY,
    site_id         BIGINT       NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    url_id          BIGINT       REFERENCES urls(id) ON DELETE CASCADE, -- NULL for SYNC_SITEMAP
    sitemap_id      BIGINT       REFERENCES sitemaps(id) ON DELETE SET NULL,
    task_type       VARCHAR(50)  NOT NULL, -- SYNC_SITEMAP | CHECK_URL | SUBMIT_URL | RETRY_SUBMISSION
    status          VARCHAR(50)  NOT NULL DEFAULT 'PENDING', -- PENDING | PROCESSING | SUCCESS | FAILED
    priority        INTEGER      NOT NULL DEFAULT 100,
    scheduled_at    TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    started_at      TIMESTAMPTZ,
    finished_at     TIMESTAMPTZ,
    retry_count     INTEGER      NOT NULL DEFAULT 0,
    locked_at       TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tasks_claim
    ON tasks(status, scheduled_at, priority)
    WHERE status = 'PENDING';
CREATE INDEX IF NOT EXISTS idx_tasks_site_id ON tasks(site_id);
CREATE INDEX IF NOT EXISTS idx_tasks_url_id ON tasks(url_id);
CREATE INDEX IF NOT EXISTS idx_tasks_type_status ON tasks(task_type, status);

-- Avoid duplicate pending work for the same URL+type
CREATE UNIQUE INDEX IF NOT EXISTS uq_tasks_pending_url_type
    ON tasks(url_id, task_type)
    WHERE status = 'PENDING' AND url_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_tasks_pending_sitemap_sync
    ON tasks(site_id, sitemap_id, task_type)
    WHERE status = 'PENDING' AND task_type = 'SYNC_SITEMAP' AND sitemap_id IS NOT NULL;

-- 5. health_checks (event history)
CREATE TABLE IF NOT EXISTS health_checks (
    id              BIGSERIAL PRIMARY KEY,
    url_id          BIGINT       NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    http_status     INTEGER,
    response_time   INTEGER, -- milliseconds
    has_noindex     BOOLEAN      NOT NULL DEFAULT FALSE,
    has_canonical   BOOLEAN      NOT NULL DEFAULT FALSE,
    checked_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_health_checks_url_id ON health_checks(url_id, checked_at DESC);

-- 6. submission_logs (search engine interaction history)
CREATE TABLE IF NOT EXISTS submission_logs (
    id              BIGSERIAL PRIMARY KEY,
    url_id          BIGINT       NOT NULL REFERENCES urls(id) ON DELETE CASCADE,
    provider        VARCHAR(50)  NOT NULL, -- google | bing
    success         BOOLEAN      NOT NULL DEFAULT FALSE,
    response_code   INTEGER,
    response_body   TEXT,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_submission_logs_url_id ON submission_logs(url_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_submission_logs_provider_time ON submission_logs(provider, created_at DESC);
