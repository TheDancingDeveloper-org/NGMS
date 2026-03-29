-- RSS feed subscriptions
CREATE TABLE rss_feeds (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    url         TEXT NOT NULL,
    protocol    TEXT NOT NULL,                      -- 'usenet' or 'torrent'
    poll_interval_secs INTEGER NOT NULL DEFAULT 900,
    category    TEXT,
    filter_regex TEXT,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    auto_download BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- RSS feed items (discovered entries, deduped by feed entry GUID)
CREATE TABLE rss_items (
    id              TEXT PRIMARY KEY,               -- feed entry GUID
    feed_id         BIGINT NOT NULL REFERENCES rss_feeds(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    url             TEXT,                            -- download URL (.nzb / .torrent / magnet)
    published_at    TIMESTAMPTZ,
    first_seen_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    downloaded      BOOLEAN NOT NULL DEFAULT false,
    downloaded_at   TIMESTAMPTZ,
    category        TEXT,
    size_bytes      BIGINT DEFAULT 0
);

CREATE INDEX idx_rss_items_feed_id    ON rss_items(feed_id);
CREATE INDEX idx_rss_items_first_seen ON rss_items(first_seen_at DESC);

-- RSS download rules (auto-grab matching items)
CREATE TABLE rss_rules (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    feed_ids    BIGINT[] NOT NULL,
    category    TEXT,
    priority    INTEGER NOT NULL DEFAULT 1,
    match_regex TEXT NOT NULL,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
