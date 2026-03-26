-- StackArr schema

-- Core config
CREATE TABLE app_config (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL
);

CREATE TABLE enabled_modules (
    id SERIAL PRIMARY KEY,
    module TEXT UNIQUE NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    config JSONB
);

CREATE TABLE media_library_folders (
    id SERIAL PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL,
    free_space BIGINT,
    last_checked TIMESTAMPTZ
);

CREATE TABLE tags (
    id SERIAL PRIMARY KEY,
    label TEXT NOT NULL UNIQUE
);

-- Quality system
CREATE TABLE quality_profiles (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    cutoff INTEGER NOT NULL,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT true,
    min_format_score INTEGER NOT NULL DEFAULT 0,
    cutoff_format_score INTEGER NOT NULL DEFAULT 0,
    items JSONB NOT NULL
);

CREATE TABLE custom_formats (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    specifications JSONB NOT NULL
);

CREATE TABLE custom_format_scores (
    profile_id INTEGER REFERENCES quality_profiles(id) ON DELETE CASCADE,
    format_id INTEGER REFERENCES custom_formats(id) ON DELETE CASCADE,
    score INTEGER NOT NULL,
    PRIMARY KEY (profile_id, format_id)
);

-- TV Series
CREATE TABLE series (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    overview TEXT,
    status TEXT NOT NULL DEFAULT 'continuing',
    series_type TEXT NOT NULL DEFAULT 'standard',
    network TEXT,
    air_time TIME,
    first_aired DATE,
    year INTEGER,
    runtime INTEGER,
    path TEXT NOT NULL,
    media_library_folder_id INTEGER REFERENCES media_library_folders(id),
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    season_folder BOOLEAN NOT NULL DEFAULT true,
    monitored BOOLEAN NOT NULL DEFAULT true,
    use_scene_numbering BOOLEAN NOT NULL DEFAULT false,
    tvdb_id BIGINT,
    imdb_id TEXT,
    tmdb_id BIGINT,
    tvmaze_id BIGINT,
    mal_id BIGINT,
    images JSONB,
    genres TEXT[],
    tags INTEGER[],
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ,
    plex_rating_key TEXT,
    plex_rating_key_4k TEXT,
    media_added_at TIMESTAMPTZ
);
CREATE INDEX idx_series_tvdb ON series(tvdb_id);
CREATE INDEX idx_series_tmdb ON series(tmdb_id);
CREATE INDEX idx_series_imdb ON series(imdb_id);
CREATE INDEX idx_series_clean_title ON series(clean_title);

CREATE TABLE seasons (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(series_id, season_number)
);

-- Media files (shared TV + movies)
CREATE TABLE media_files (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    size BIGINT NOT NULL,
    date_added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quality JSONB NOT NULL,
    languages JSONB NOT NULL DEFAULT '[]',
    scene_name TEXT,
    release_group TEXT,
    release_hash TEXT,
    edition TEXT,
    media_info JSONB,
    indexer_flags INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE episodes (
    id BIGSERIAL PRIMARY KEY,
    series_id BIGINT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    absolute_number INTEGER,
    scene_season_number INTEGER,
    scene_episode_number INTEGER,
    scene_absolute_number INTEGER,
    title TEXT,
    overview TEXT,
    air_date DATE,
    air_date_utc TIMESTAMPTZ,
    runtime INTEGER,
    monitored BOOLEAN NOT NULL DEFAULT true,
    episode_file_id BIGINT REFERENCES media_files(id) ON DELETE SET NULL,
    last_search_time TIMESTAMPTZ,
    UNIQUE(series_id, season_number, episode_number)
);
CREATE INDEX idx_episodes_air_date ON episodes(air_date_utc);

-- Episode-to-file join (multi-episode files)
CREATE TABLE episode_files (
    episode_id BIGINT NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    media_file_id BIGINT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    PRIMARY KEY (episode_id, media_file_id)
);

-- Movies
CREATE TABLE movies (
    id BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    overview TEXT,
    year INTEGER,
    studio TEXT,
    path TEXT NOT NULL,
    media_library_folder_id INTEGER REFERENCES media_library_folders(id),
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    monitored BOOLEAN NOT NULL DEFAULT true,
    minimum_availability TEXT NOT NULL DEFAULT 'released',
    movie_file_id BIGINT REFERENCES media_files(id) ON DELETE SET NULL,
    tmdb_id BIGINT,
    imdb_id TEXT,
    in_cinemas DATE,
    physical_release DATE,
    digital_release DATE,
    images JSONB,
    genres TEXT[],
    tags INTEGER[],
    collection_tmdb_id BIGINT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ,
    plex_rating_key TEXT,
    plex_rating_key_4k TEXT,
    media_added_at TIMESTAMPTZ
);
CREATE INDEX idx_movies_tmdb ON movies(tmdb_id);
CREATE INDEX idx_movies_imdb ON movies(imdb_id);
CREATE INDEX idx_movies_clean_title ON movies(clean_title);

-- Alternative titles
CREATE TABLE alternative_titles (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id BIGINT NOT NULL,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    scene_name BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX idx_alt_titles_clean ON alternative_titles(clean_title);

-- Indexers
CREATE TABLE indexers (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    indexer_type TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    protocol TEXT NOT NULL,
    categories INTEGER[],
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 25,
    supports_search BOOLEAN NOT NULL DEFAULT true,
    supports_rss BOOLEAN NOT NULL DEFAULT true,
    config JSONB,
    last_rss_sync TIMESTAMPTZ
);

-- Download clients
CREATE TABLE download_clients (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    client_type TEXT NOT NULL,
    protocol TEXT NOT NULL,
    config JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 1
);

-- Queue (tracked in-progress downloads)
CREATE TABLE queue (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id BIGINT NOT NULL,
    episode_id BIGINT,
    title TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    size BIGINT,
    status TEXT NOT NULL,
    download_id TEXT NOT NULL,
    download_client_id INTEGER REFERENCES download_clients(id),
    indexer_id INTEGER REFERENCES indexers(id),
    protocol TEXT NOT NULL,
    error_message TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_queue_download_id ON queue(download_id);

-- History
CREATE TABLE history (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id BIGINT NOT NULL,
    episode_id BIGINT,
    event_type TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    source_title TEXT NOT NULL,
    download_id TEXT,
    indexer_id INTEGER,
    download_client TEXT,
    data JSONB,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_history_media ON history(media_type, media_id);
CREATE INDEX idx_history_occurred ON history(occurred_at DESC);
CREATE INDEX idx_history_download_id ON history(download_id);

-- Blocklist
CREATE TABLE blocklist (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id BIGINT NOT NULL,
    source_title TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    indexer_id INTEGER,
    info_hash TEXT,
    message TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_blocklist_media ON blocklist(media_type, media_id);
CREATE INDEX idx_blocklist_hash ON blocklist(info_hash);

-- Naming config
CREATE TABLE naming_config (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL UNIQUE,
    rename_files BOOLEAN NOT NULL DEFAULT true,
    standard_format TEXT,
    daily_format TEXT,
    anime_format TEXT,
    season_folder_format TEXT,
    movie_format TEXT,
    movie_folder_format TEXT,
    colon_replacement TEXT NOT NULL DEFAULT 'smart'
);

-- Notification providers
CREATE TABLE notification_providers (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    config JSONB NOT NULL,
    on_grab BOOLEAN NOT NULL DEFAULT false,
    on_import BOOLEAN NOT NULL DEFAULT false,
    on_upgrade BOOLEAN NOT NULL DEFAULT false,
    on_health_issue BOOLEAN NOT NULL DEFAULT false,
    on_failure BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true
);

-- Import lists
CREATE TABLE import_lists (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    list_type TEXT NOT NULL,
    media_type TEXT NOT NULL,
    config JSONB NOT NULL,
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    media_library_folder_id INTEGER REFERENCES media_library_folders(id),
    monitored BOOLEAN NOT NULL DEFAULT true,
    enabled BOOLEAN NOT NULL DEFAULT true,
    poll_interval_secs INTEGER NOT NULL DEFAULT 3600
);

-- Discover sliders
CREATE TABLE discover_sliders (
    id SERIAL PRIMARY KEY,
    slider_type TEXT NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 0,
    is_built_in BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true,
    title TEXT,
    custom_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_discover_sliders_order ON discover_sliders(display_order);

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

CREATE TABLE plex_libraries (
    id SERIAL PRIMARY KEY,
    plex_server_id INTEGER NOT NULL REFERENCES plex_servers(id) ON DELETE CASCADE,
    section_id TEXT NOT NULL,
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    library_type TEXT NOT NULL,
    last_scan TIMESTAMPTZ,
    UNIQUE(plex_server_id, section_id)
);

CREATE TABLE watchlist (
    id BIGSERIAL PRIMARY KEY,
    tmdb_id BIGINT NOT NULL,
    media_type TEXT NOT NULL,
    plex_rating_key TEXT,
    auto_requested BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tmdb_id, media_type)
);
CREATE INDEX idx_watchlist_tmdb ON watchlist(tmdb_id);

-- Seed default quality profiles
INSERT INTO quality_profiles (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items) VALUES
('Any', 20, true, 0, 0, '[{"quality": 1, "allowed": true}, {"quality": 2, "allowed": true}, {"quality": 3, "allowed": true}, {"quality": 4, "allowed": true}, {"quality": 5, "allowed": true}, {"quality": 6, "allowed": true}, {"quality": 7, "allowed": true}, {"quality": 8, "allowed": true}, {"quality": 9, "allowed": true}, {"quality": 10, "allowed": true}, {"quality": 11, "allowed": true}, {"quality": 12, "allowed": true}, {"quality": 13, "allowed": true}, {"quality": 14, "allowed": true}, {"quality": 15, "allowed": true}, {"quality": 16, "allowed": true}, {"quality": 17, "allowed": true}, {"quality": 18, "allowed": true}, {"quality": 19, "allowed": true}]'),
('HD-1080p', 13, true, 0, 0, '[{"quality": 10, "allowed": true}, {"quality": 11, "allowed": true}, {"quality": 12, "allowed": true}, {"quality": 13, "allowed": true}, {"quality": 14, "allowed": true}]'),
('Ultra-HD', 18, true, 0, 0, '[{"quality": 15, "allowed": true}, {"quality": 16, "allowed": true}, {"quality": 17, "allowed": true}, {"quality": 18, "allowed": true}, {"quality": 19, "allowed": true}]');

-- Seed default naming config
INSERT INTO naming_config (media_type, standard_format, daily_format, anime_format, season_folder_format, colon_replacement) VALUES
('series', '{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]', '{Series Title} - {Air-Date} - {Episode Title} [{Quality Title}]', '{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]', 'Season {season:00}', 'smart');

INSERT INTO naming_config (media_type, movie_format, movie_folder_format, colon_replacement) VALUES
('movie', '{Movie Title} ({Release Year}) [{Quality Title}]', '{Movie Title} ({Release Year})', 'smart');

-- Seed default discover sliders
INSERT INTO discover_sliders (slider_type, display_order, is_built_in, enabled, title) VALUES
('trending',           1,  true, true, 'Trending'),
('popular_movies',     2,  true, true, 'Popular Movies'),
('popular_tv',         3,  true, true, 'Popular TV Shows'),
('upcoming_movies',    4,  true, true, 'Upcoming Movies'),
('upcoming_tv',        5,  true, true, 'Upcoming TV Shows'),
('recently_added',     6,  true, true, 'Recently Added'),
('movie_genres',       7,  true, true, 'Movie Genres'),
('tv_genres',          8,  true, true, 'TV Genres');
