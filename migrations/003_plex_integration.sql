-- Plex server connections
CREATE TABLE plex_servers (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL DEFAULT 'Plex',
    machine_id TEXT,
    ip TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 32400,
    use_ssl BOOLEAN NOT NULL DEFAULT false,
    auth_token TEXT,
    web_app_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Plex libraries (sections) linked to a server
CREATE TABLE plex_libraries (
    id SERIAL PRIMARY KEY,
    plex_server_id INTEGER NOT NULL REFERENCES plex_servers(id) ON DELETE CASCADE,
    section_id TEXT NOT NULL,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    library_type TEXT NOT NULL, -- 'movie' or 'show'
    last_scan TIMESTAMPTZ,
    UNIQUE(plex_server_id, section_id)
);

-- Watchlist entries synced from Plex
CREATE TABLE watchlist (
    id BIGSERIAL PRIMARY KEY,
    tmdb_id BIGINT NOT NULL,
    media_type TEXT NOT NULL, -- 'movie' or 'tv'
    plex_rating_key TEXT,
    auto_requested BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tmdb_id, media_type)
);

CREATE INDEX idx_watchlist_tmdb ON watchlist(tmdb_id);

-- Add Plex tracking columns to series and movies
ALTER TABLE series ADD COLUMN plex_rating_key TEXT;
ALTER TABLE series ADD COLUMN plex_rating_key_4k TEXT;
ALTER TABLE series ADD COLUMN media_added_at TIMESTAMPTZ;

ALTER TABLE movies ADD COLUMN plex_rating_key TEXT;
ALTER TABLE movies ADD COLUMN plex_rating_key_4k TEXT;
ALTER TABLE movies ADD COLUMN media_added_at TIMESTAMPTZ;
