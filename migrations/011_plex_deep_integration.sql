-- Plex deep integration: events, webhook secrets, unified streaming

-- Plex webhook event history
CREATE TABLE plex_events (
    id BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    plex_server_id INTEGER REFERENCES plex_servers(id) ON DELETE SET NULL,
    user_name TEXT,
    title TEXT,
    rating_key TEXT,
    metadata JSONB,
    thumb_url TEXT,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_plex_events_type ON plex_events(event_type);
CREATE INDEX idx_plex_events_received ON plex_events(received_at DESC);

-- Webhook secret per Plex server (used in webhook URL path for validation)
ALTER TABLE plex_servers ADD COLUMN IF NOT EXISTS webhook_secret TEXT;
