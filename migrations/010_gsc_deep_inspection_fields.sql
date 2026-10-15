ALTER TABLE urls ADD COLUMN gsc_verdict TEXT;
ALTER TABLE urls ADD COLUMN gsc_crawled_as TEXT;
ALTER TABLE urls ADD COLUMN gsc_page_fetch_state TEXT;
ALTER TABLE urls ADD COLUMN gsc_robots_txt_state TEXT;
ALTER TABLE urls ADD COLUMN gsc_indexing_state TEXT;
ALTER TABLE urls ADD COLUMN gsc_user_canonical TEXT;
ALTER TABLE urls ADD COLUMN gsc_google_canonical TEXT;
ALTER TABLE urls ADD COLUMN gsc_referring_urls TEXT;
