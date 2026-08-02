-- SPDX-License-Identifier: GPL-3.0-only
-- StackArr fresh-deploy baseline for MariaDB 11.4 LTS.
-- Datetimes are UTC DATETIME(6); JSON replaces PostgreSQL arrays and JSONB.

CREATE TABLE app_config (
    `key` VARCHAR(191) PRIMARY KEY,
    value JSON NOT NULL
) ENGINE=InnoDB;

CREATE TABLE enabled_modules (
    id INT AUTO_INCREMENT PRIMARY KEY,
    module VARCHAR(191) NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    config JSON
) ENGINE=InnoDB;

CREATE TABLE media_library_folders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    path VARCHAR(2048) NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    free_space BIGINT,
    last_checked DATETIME(6),
    UNIQUE KEY uq_media_library_folder_path (path(768))
) ENGINE=InnoDB;

CREATE TABLE tags (
    id INT AUTO_INCREMENT PRIMARY KEY,
    label VARCHAR(191) NOT NULL UNIQUE
) ENGINE=InnoDB;

CREATE TABLE quality_profiles (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    cutoff INT NOT NULL,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT TRUE,
    min_format_score INT NOT NULL DEFAULT 0,
    cutoff_format_score INT NOT NULL DEFAULT 0,
    min_upgrade_format_score INT NOT NULL DEFAULT 1,
    items JSON NOT NULL,
    media_type VARCHAR(32),
    language INT NOT NULL DEFAULT -1
) ENGINE=InnoDB;

CREATE TABLE custom_formats (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    specifications JSON NOT NULL,
    include_custom_format_when_renaming BOOLEAN NOT NULL DEFAULT FALSE
) ENGINE=InnoDB;

CREATE TABLE custom_format_scores (
    profile_id INT NOT NULL,
    format_id INT NOT NULL,
    score INT NOT NULL,
    PRIMARY KEY (profile_id, format_id),
    CONSTRAINT fk_custom_format_scores_profile FOREIGN KEY (profile_id) REFERENCES quality_profiles(id) ON DELETE CASCADE,
    CONSTRAINT fk_custom_format_scores_format FOREIGN KEY (format_id) REFERENCES custom_formats(id) ON DELETE CASCADE
) ENGINE=InnoDB;

-- Generic identity shared by all present and future media-type adapters.
CREATE TABLE media_entities (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    source_key VARCHAR(255) NOT NULL,
    title VARCHAR(1024) NOT NULL,
    sort_title VARCHAR(1024) NOT NULL,
    year INT,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    library_folder_id INT,
    quality_profile_id INT,
    external_ids JSON NOT NULL,
    metadata JSON NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_media_entity_source (media_type, source_key),
    KEY idx_media_entities_type_title (media_type, title(191)),
    CONSTRAINT fk_media_entities_folder FOREIGN KEY (library_folder_id) REFERENCES media_library_folders(id) ON DELETE SET NULL,
    CONSTRAINT fk_media_entities_profile FOREIGN KEY (quality_profile_id) REFERENCES quality_profiles(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE series (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    entity_id BIGINT UNIQUE,
    title VARCHAR(1024) NOT NULL,
    clean_title VARCHAR(1024) NOT NULL,
    sort_title VARCHAR(1024) NOT NULL,
    overview LONGTEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'continuing',
    series_type VARCHAR(32) NOT NULL DEFAULT 'standard',
    network VARCHAR(255),
    air_time TIME,
    first_aired DATE,
    year INT,
    runtime INT,
    path VARCHAR(2048) NOT NULL,
    media_library_folder_id INT,
    quality_profile_id INT,
    season_folder BOOLEAN NOT NULL DEFAULT TRUE,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    use_scene_numbering BOOLEAN NOT NULL DEFAULT FALSE,
    tvdb_id BIGINT,
    imdb_id VARCHAR(32),
    tmdb_id BIGINT,
    tvmaze_id BIGINT,
    mal_id BIGINT,
    images JSON,
    genres JSON,
    tags JSON,
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_info_sync DATETIME(6),
    plex_rating_key VARCHAR(255),
    plex_rating_key_4k VARCHAR(255),
    media_added_at DATETIME(6),
    KEY idx_series_tvdb (tvdb_id),
    KEY idx_series_tmdb (tmdb_id),
    KEY idx_series_imdb (imdb_id),
    KEY idx_series_clean_title (clean_title(191)),
    CONSTRAINT fk_series_entity FOREIGN KEY (entity_id) REFERENCES media_entities(id) ON DELETE SET NULL,
    CONSTRAINT fk_series_folder FOREIGN KEY (media_library_folder_id) REFERENCES media_library_folders(id) ON DELETE SET NULL,
    CONSTRAINT fk_series_profile FOREIGN KEY (quality_profile_id) REFERENCES quality_profiles(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE seasons (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    series_id BIGINT NOT NULL,
    season_number INT NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE KEY uq_season (series_id, season_number),
    CONSTRAINT fk_seasons_series FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE media_files (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    relative_path VARCHAR(2048) NOT NULL,
    size BIGINT NOT NULL,
    date_added DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    quality JSON NOT NULL,
    languages JSON NOT NULL,
    scene_name VARCHAR(1024),
    release_group VARCHAR(255),
    release_hash VARCHAR(255),
    edition VARCHAR(255),
    media_info JSON,
    indexer_flags INT NOT NULL DEFAULT 0,
    KEY idx_media_files_media_type (media_type)
) ENGINE=InnoDB;

CREATE TABLE episodes (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    series_id BIGINT NOT NULL,
    season_number INT NOT NULL,
    episode_number INT NOT NULL,
    absolute_number INT,
    scene_season_number INT,
    scene_episode_number INT,
    scene_absolute_number INT,
    title VARCHAR(1024),
    overview LONGTEXT,
    air_date DATE,
    air_date_utc DATETIME(6),
    runtime INT,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    episode_file_id BIGINT,
    last_search_time DATETIME(6),
    UNIQUE KEY uq_episode (series_id, season_number, episode_number),
    KEY idx_episodes_air_date (air_date_utc),
    CONSTRAINT fk_episodes_series FOREIGN KEY (series_id) REFERENCES series(id) ON DELETE CASCADE,
    CONSTRAINT fk_episodes_file FOREIGN KEY (episode_file_id) REFERENCES media_files(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE episode_files (
    episode_id BIGINT NOT NULL,
    media_file_id BIGINT NOT NULL,
    PRIMARY KEY (episode_id, media_file_id),
    KEY idx_episode_files_media_file (media_file_id),
    CONSTRAINT fk_episode_files_episode FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE,
    CONSTRAINT fk_episode_files_media FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE movies (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    entity_id BIGINT UNIQUE,
    title VARCHAR(1024) NOT NULL,
    clean_title VARCHAR(1024) NOT NULL,
    sort_title VARCHAR(1024) NOT NULL,
    overview LONGTEXT,
    year INT,
    studio VARCHAR(255),
    path VARCHAR(2048) NOT NULL,
    media_library_folder_id INT,
    quality_profile_id INT,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    minimum_availability VARCHAR(32) NOT NULL DEFAULT 'released',
    movie_file_id BIGINT,
    tmdb_id BIGINT,
    imdb_id VARCHAR(32),
    in_cinemas DATE,
    physical_release DATE,
    digital_release DATE,
    images JSON,
    genres JSON,
    tags JSON,
    collection_tmdb_id BIGINT,
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_info_sync DATETIME(6),
    plex_rating_key VARCHAR(255),
    plex_rating_key_4k VARCHAR(255),
    media_added_at DATETIME(6),
    original_language INT,
    KEY idx_movies_tmdb (tmdb_id),
    KEY idx_movies_imdb (imdb_id),
    KEY idx_movies_clean_title (clean_title(191)),
    KEY idx_movies_movie_file (movie_file_id),
    CONSTRAINT fk_movies_entity FOREIGN KEY (entity_id) REFERENCES media_entities(id) ON DELETE SET NULL,
    CONSTRAINT fk_movies_folder FOREIGN KEY (media_library_folder_id) REFERENCES media_library_folders(id) ON DELETE SET NULL,
    CONSTRAINT fk_movies_profile FOREIGN KEY (quality_profile_id) REFERENCES quality_profiles(id) ON DELETE SET NULL,
    CONSTRAINT fk_movies_file FOREIGN KEY (movie_file_id) REFERENCES media_files(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE alternative_titles (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    title VARCHAR(1024) NOT NULL,
    clean_title VARCHAR(1024) NOT NULL,
    scene_name BOOLEAN NOT NULL DEFAULT FALSE,
    KEY idx_alt_titles_clean (clean_title(191))
) ENGINE=InnoDB;

CREATE TABLE indexers (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    indexer_type VARCHAR(64) NOT NULL,
    base_url VARCHAR(2048) NOT NULL,
    api_key TEXT,
    protocol VARCHAR(32) NOT NULL,
    categories JSON,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    priority INT NOT NULL DEFAULT 25,
    supports_search BOOLEAN NOT NULL DEFAULT TRUE,
    supports_rss BOOLEAN NOT NULL DEFAULT TRUE,
    config JSON,
    last_rss_sync DATETIME(6),
    last_health_check DATETIME(6),
    health_status VARCHAR(32) NOT NULL DEFAULT 'unknown',
    consecutive_failures INT NOT NULL DEFAULT 0,
    auto_disabled BOOLEAN NOT NULL DEFAULT FALSE
) ENGINE=InnoDB;

CREATE TABLE download_clients (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    client_type VARCHAR(64) NOT NULL,
    protocol VARCHAR(32) NOT NULL,
    config JSON NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    priority INT NOT NULL DEFAULT 1,
    last_health_check DATETIME(6),
    health_status VARCHAR(32) NOT NULL DEFAULT 'unknown',
    consecutive_failures INT NOT NULL DEFAULT 0,
    auto_disabled BOOLEAN NOT NULL DEFAULT FALSE
) ENGINE=InnoDB;

CREATE TABLE queue (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    episode_id BIGINT,
    title VARCHAR(2048) NOT NULL,
    quality JSON NOT NULL,
    languages JSON,
    size BIGINT,
    status VARCHAR(64) NOT NULL,
    download_id VARCHAR(255) NOT NULL,
    download_client_id INT,
    indexer_id INT,
    protocol VARCHAR(32) NOT NULL,
    error_message LONGTEXT,
    output_path VARCHAR(2048),
    stale_count INT NOT NULL DEFAULT 0,
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_queue_download_id (download_id),
    KEY idx_queue_media (media_type, media_id),
    CONSTRAINT fk_queue_client FOREIGN KEY (download_client_id) REFERENCES download_clients(id) ON DELETE SET NULL,
    CONSTRAINT fk_queue_indexer FOREIGN KEY (indexer_id) REFERENCES indexers(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE history (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    episode_id BIGINT,
    event_type VARCHAR(64) NOT NULL,
    quality JSON NOT NULL,
    languages JSON,
    source_title VARCHAR(2048) NOT NULL,
    download_id VARCHAR(255),
    indexer_id INT,
    download_client VARCHAR(255),
    data JSON,
    occurred_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_history_media (media_type, media_id),
    KEY idx_history_occurred (occurred_at DESC),
    KEY idx_history_download_id (download_id)
) ENGINE=InnoDB;

CREATE TABLE blocklist (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    source_title VARCHAR(2048) NOT NULL,
    quality JSON NOT NULL,
    languages JSON,
    indexer_id INT,
    info_hash VARCHAR(255),
    message LONGTEXT,
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_blocklist_media (media_type, media_id),
    KEY idx_blocklist_hash (info_hash)
) ENGINE=InnoDB;

CREATE TABLE naming_config (
    id INT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL UNIQUE,
    rename_files BOOLEAN NOT NULL DEFAULT TRUE,
    standard_format TEXT,
    daily_format TEXT,
    anime_format TEXT,
    season_folder_format TEXT,
    movie_format TEXT,
    movie_folder_format TEXT,
    colon_replacement VARCHAR(32) NOT NULL DEFAULT 'smart'
) ENGINE=InnoDB;

CREATE TABLE notification_providers (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    provider_type VARCHAR(64) NOT NULL,
    config JSON NOT NULL,
    on_grab BOOLEAN NOT NULL DEFAULT FALSE,
    on_import BOOLEAN NOT NULL DEFAULT FALSE,
    on_upgrade BOOLEAN NOT NULL DEFAULT FALSE,
    on_health_issue BOOLEAN NOT NULL DEFAULT FALSE,
    on_failure BOOLEAN NOT NULL DEFAULT FALSE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
) ENGINE=InnoDB;

CREATE TABLE import_lists (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    list_type VARCHAR(64) NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    config JSON NOT NULL,
    quality_profile_id INT,
    media_library_folder_id INT,
    monitored BOOLEAN NOT NULL DEFAULT TRUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    poll_interval_secs INT NOT NULL DEFAULT 3600,
    CONSTRAINT fk_import_lists_profile FOREIGN KEY (quality_profile_id) REFERENCES quality_profiles(id) ON DELETE SET NULL,
    CONSTRAINT fk_import_lists_folder FOREIGN KEY (media_library_folder_id) REFERENCES media_library_folders(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE discover_sliders (
    id INT AUTO_INCREMENT PRIMARY KEY,
    slider_type VARCHAR(64) NOT NULL,
    display_order INT NOT NULL DEFAULT 0,
    is_built_in BOOLEAN NOT NULL DEFAULT FALSE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    title VARCHAR(255),
    custom_data JSON,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_discover_sliders_order (display_order)
) ENGINE=InnoDB;

CREATE TABLE plex_servers (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL DEFAULT 'Plex',
    machine_id VARCHAR(255),
    ip VARCHAR(255) NOT NULL,
    port INT NOT NULL DEFAULT 32400,
    use_ssl BOOLEAN NOT NULL DEFAULT FALSE,
    verify_tls BOOLEAN NOT NULL DEFAULT FALSE,
    auth_token TEXT,
    web_app_url VARCHAR(2048),
    webhook_secret VARCHAR(255),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB;

CREATE TABLE plex_libraries (
    id INT AUTO_INCREMENT PRIMARY KEY,
    plex_server_id INT NOT NULL,
    section_id VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    library_type VARCHAR(64) NOT NULL,
    last_scan DATETIME(6),
    UNIQUE KEY uq_plex_library (plex_server_id, section_id),
    CONSTRAINT fk_plex_libraries_server FOREIGN KEY (plex_server_id) REFERENCES plex_servers(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE plex_events (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    event_type VARCHAR(128) NOT NULL,
    plex_server_id INT,
    user_name VARCHAR(255),
    title VARCHAR(1024),
    rating_key VARCHAR(255),
    metadata JSON,
    thumb_url VARCHAR(2048),
    received_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_plex_events_type (event_type),
    KEY idx_plex_events_received (received_at DESC),
    CONSTRAINT fk_plex_events_server FOREIGN KEY (plex_server_id) REFERENCES plex_servers(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE watchlist (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    tmdb_id BIGINT NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    plex_rating_key VARCHAR(255),
    auto_requested BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_watchlist_media (tmdb_id, media_type),
    KEY idx_watchlist_tmdb (tmdb_id)
) ENGINE=InnoDB;

CREATE TABLE streaming_sessions (
    id CHAR(36) PRIMARY KEY,
    media_file_id BIGINT NOT NULL,
    session_type VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    started_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_activity DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    transcode_progress FLOAT,
    video_codec VARCHAR(64),
    audio_codec VARCHAR(64),
    resolution VARCHAR(64),
    bitrate BIGINT,
    client_info TEXT,
    transcode_dir VARCHAR(2048),
    user_id BIGINT,
    KEY idx_streaming_sessions_media (media_file_id),
    KEY idx_streaming_sessions_status (status),
    CONSTRAINT fk_streaming_sessions_media FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE remote_clients (
    id INT AUTO_INCREMENT PRIMARY KEY,
    client_token CHAR(36) NOT NULL UNIQUE,
    client_name VARCHAR(255),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_seen DATETIME(6),
    revoked BOOLEAN NOT NULL DEFAULT FALSE
) ENGINE=InnoDB;

CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    username VARCHAR(191) NOT NULL UNIQUE,
    display_name VARCHAR(255) NOT NULL,
    password_hash TEXT NOT NULL,
    role VARCHAR(32) NOT NULL DEFAULT 'user',
    avatar_url VARCHAR(2048),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB;

ALTER TABLE streaming_sessions
    ADD CONSTRAINT fk_streaming_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

CREATE TABLE user_sessions (
    id CHAR(36) PRIMARY KEY,
    user_id BIGINT NOT NULL,
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    user_agent TEXT,
    ip_address VARCHAR(45),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    expires_at DATETIME(6) NOT NULL,
    last_active DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_user_sessions_user (user_id),
    KEY idx_user_sessions_expires (expires_at),
    CONSTRAINT fk_user_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE user_devices (
    id INT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    device_token CHAR(36) NOT NULL UNIQUE,
    device_name VARCHAR(255),
    device_type VARCHAR(64),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    last_seen DATETIME(6),
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT fk_user_devices_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE invites (
    id INT AUTO_INCREMENT PRIMARY KEY,
    code VARCHAR(191) NOT NULL UNIQUE,
    created_by BIGINT NOT NULL,
    claimed_by BIGINT,
    role VARCHAR(32) NOT NULL DEFAULT 'user',
    expires_at DATETIME(6),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_invites_creator FOREIGN KEY (created_by) REFERENCES users(id),
    CONSTRAINT fk_invites_claimant FOREIGN KEY (claimed_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE watch_progress (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    media_file_id BIGINT NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    episode_id BIGINT,
    position_secs FLOAT NOT NULL DEFAULT 0,
    duration_secs FLOAT NOT NULL DEFAULT 0,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_watch_progress_file (user_id, media_file_id),
    KEY idx_watch_progress_user (user_id, updated_at DESC),
    KEY idx_watch_progress_continue (user_id, completed, updated_at DESC),
    KEY idx_watch_progress_media (media_type, media_id),
    CONSTRAINT fk_watch_progress_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT fk_watch_progress_file FOREIGN KEY (media_file_id) REFERENCES media_files(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE media_requests (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    tmdb_id BIGINT NOT NULL,
    title VARCHAR(1024) NOT NULL,
    year INT,
    poster_url VARCHAR(2048),
    overview LONGTEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    admin_note LONGTEXT,
    approved_by BIGINT,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_media_request (tmdb_id, media_type),
    KEY idx_media_requests_user (user_id),
    KEY idx_media_requests_status (status),
    CONSTRAINT fk_media_requests_user FOREIGN KEY (user_id) REFERENCES users(id),
    CONSTRAINT fk_media_requests_approver FOREIGN KEY (approved_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE user_watchlist (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    tmdb_id BIGINT NOT NULL,
    added_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_user_watchlist (user_id, media_type, media_id),
    KEY idx_user_watchlist_user (user_id, added_at DESC),
    CONSTRAINT fk_user_watchlist_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE user_ratings (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    rating SMALLINT NOT NULL CHECK (rating BETWEEN 1 AND 10),
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_user_rating (user_id, media_type, media_id),
    KEY idx_user_ratings_media (media_type, media_id),
    CONSTRAINT fk_user_ratings_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE user_notifications (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    notification_type VARCHAR(64) NOT NULL,
    title VARCHAR(1024) NOT NULL,
    body LONGTEXT,
    data JSON,
    `read` BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_user_notifications_user (user_id, `read`, created_at DESC),
    KEY idx_user_notifications_created (created_at),
    CONSTRAINT fk_user_notifications_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE push_subscriptions (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id BIGINT NOT NULL,
    endpoint VARCHAR(2048) NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    user_agent TEXT,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_push_subscription_endpoint (endpoint(768)),
    CONSTRAINT fk_push_subscriptions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE recycle_bin (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    original_path VARCHAR(2048) NOT NULL,
    recycle_path VARCHAR(2048) NOT NULL,
    media_file_id BIGINT,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    recycled_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_recycle_bin_recycled_at (recycled_at)
) ENGINE=InnoDB;

CREATE TABLE system_activities (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    activity_type VARCHAR(64) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'running',
    title VARCHAR(1024) NOT NULL,
    detail LONGTEXT,
    progress JSON,
    result JSON,
    error LONGTEXT,
    started_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    completed_at DATETIME(6),
    KEY idx_system_activities_status (status, started_at DESC),
    KEY idx_system_activities_recent (started_at DESC)
) ENGINE=InnoDB;

CREATE TABLE rss_feeds (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    url VARCHAR(2048) NOT NULL,
    protocol VARCHAR(32) NOT NULL,
    poll_interval_secs INT NOT NULL DEFAULT 900,
    category VARCHAR(255),
    filter_regex TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    auto_download BOOLEAN NOT NULL DEFAULT FALSE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB;

CREATE TABLE rss_items (
    id VARCHAR(768) PRIMARY KEY,
    feed_id BIGINT NOT NULL,
    title VARCHAR(2048) NOT NULL,
    url VARCHAR(2048),
    published_at DATETIME(6),
    first_seen_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    downloaded BOOLEAN NOT NULL DEFAULT FALSE,
    downloaded_at DATETIME(6),
    category VARCHAR(255),
    size_bytes BIGINT DEFAULT 0,
    KEY idx_rss_items_feed_id (feed_id),
    KEY idx_rss_items_first_seen (first_seen_at DESC),
    CONSTRAINT fk_rss_items_feed FOREIGN KEY (feed_id) REFERENCES rss_feeds(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE rss_rules (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    feed_ids JSON NOT NULL,
    category VARCHAR(255),
    priority INT NOT NULL DEFAULT 1,
    match_regex TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB;

CREATE TABLE dav_items (
    id CHAR(36) PRIMARY KEY,
    id_prefix VARCHAR(64) NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    parent_id CHAR(36),
    name VARCHAR(1024) NOT NULL,
    file_size BIGINT,
    item_type INT NOT NULL,
    sub_type INT NOT NULL,
    path VARCHAR(2048) NOT NULL,
    release_date DATETIME(6),
    last_health_check DATETIME(6),
    next_health_check DATETIME(6),
    history_item_id CHAR(36),
    file_blob_id CHAR(36),
    nzb_blob_id CHAR(36),
    UNIQUE KEY uq_dav_item_name (parent_id, name(191)),
    KEY idx_dav_items_prefix (id_prefix, item_type),
    KEY idx_dav_items_type_created (item_type, created_at),
    KEY idx_dav_items_sub_type (sub_type, created_at),
    KEY idx_dav_items_history (history_item_id, item_type),
    KEY idx_dav_items_nzb_blob (nzb_blob_id),
    KEY idx_dav_items_path (path(768)),
    CONSTRAINT fk_dav_items_parent FOREIGN KEY (parent_id) REFERENCES dav_items(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE dav_blobs (id CHAR(36) PRIMARY KEY, data LONGBLOB NOT NULL) ENGINE=InnoDB;
CREATE TABLE dav_nzb_blobs (id CHAR(36) PRIMARY KEY, data LONGBLOB NOT NULL) ENGINE=InnoDB;

CREATE TABLE dav_queue_items (
    id CHAR(36) PRIMARY KEY,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    file_name VARCHAR(2048) NOT NULL,
    job_name VARCHAR(1024) NOT NULL,
    nzb_file_size BIGINT NOT NULL DEFAULT 0,
    total_segment_bytes BIGINT NOT NULL DEFAULT 0,
    category VARCHAR(255) NOT NULL DEFAULT '',
    priority INT NOT NULL DEFAULT 0,
    post_processing INT NOT NULL DEFAULT -1,
    pause_until DATETIME(6),
    KEY idx_dav_queue_priority (priority DESC, created_at ASC)
) ENGINE=InnoDB;

CREATE TABLE dav_history_items (
    id CHAR(36) PRIMARY KEY,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    file_name VARCHAR(2048) NOT NULL,
    job_name VARCHAR(1024) NOT NULL,
    category VARCHAR(255) NOT NULL DEFAULT '',
    download_status INT NOT NULL,
    total_segment_bytes BIGINT NOT NULL DEFAULT 0,
    download_time_seconds INT NOT NULL DEFAULT 0,
    fail_message LONGTEXT,
    download_dir_id CHAR(36),
    nzb_blob_id CHAR(36),
    KEY idx_dav_history_created (created_at)
) ENGINE=InnoDB;

CREATE TABLE dav_health_checks (
    id CHAR(36) PRIMARY KEY,
    dav_item_id CHAR(36) NOT NULL,
    path VARCHAR(2048) NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    result INT NOT NULL DEFAULT 0,
    repair_status INT NOT NULL DEFAULT 0,
    message LONGTEXT NOT NULL,
    CONSTRAINT fk_dav_health_item FOREIGN KEY (dav_item_id) REFERENCES dav_items(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE dav_config (`key` VARCHAR(191) PRIMARY KEY, value TEXT NOT NULL) ENGINE=InnoDB;

CREATE TABLE import_candidates (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_library_folder_id INT,
    media_type VARCHAR(32) NOT NULL,
    match_kind VARCHAR(32) NOT NULL,
    discovered_path VARCHAR(2048) NOT NULL,
    file_count INT NOT NULL DEFAULT 1,
    total_size BIGINT NOT NULL DEFAULT 0,
    parsed_title VARCHAR(1024),
    parsed_year INT,
    parsed_season INT,
    parsed_episodes JSON,
    suggested_tmdb_id INT,
    suggested_title VARCHAR(1024),
    suggested_year INT,
    suggested_poster VARCHAR(2048),
    suggested_overview LONGTEXT,
    confidence FLOAT NOT NULL DEFAULT 0.0,
    status VARCHAR(32) NOT NULL DEFAULT 'pending',
    target_series_id BIGINT,
    target_movie_id BIGINT,
    error LONGTEXT,
    data JSON NOT NULL,
    discovered_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    resolved_at DATETIME(6),
    pending_path_hash BINARY(32) GENERATED ALWAYS AS (
        IF(status = 'pending', UNHEX(SHA2(discovered_path, 256)), NULL)
    ) STORED,
    UNIQUE KEY uq_import_candidates_pending_path (pending_path_hash),
    KEY idx_import_candidates_status (status),
    KEY idx_import_candidates_media_type (media_type),
    KEY idx_import_candidates_discovered_path (discovered_path(768)),
    CONSTRAINT fk_import_candidates_folder FOREIGN KEY (media_library_folder_id) REFERENCES media_library_folders(id) ON DELETE CASCADE,
    CONSTRAINT fk_import_candidates_series FOREIGN KEY (target_series_id) REFERENCES series(id) ON DELETE SET NULL,
    CONSTRAINT fk_import_candidates_movie FOREIGN KEY (target_movie_id) REFERENCES movies(id) ON DELETE SET NULL
) ENGINE=InnoDB;

-- P5: upstream profile subscriptions, immutable snapshots, local overrides, provenance.
CREATE TABLE profile_sources (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    source_type VARCHAR(32) NOT NULL,
    name VARCHAR(255) NOT NULL,
    repository_url VARCHAR(2048) NOT NULL,
    reference_name VARCHAR(255) NOT NULL DEFAULT 'main',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_profile_source (repository_url(512), reference_name)
) ENGINE=InnoDB;

CREATE TABLE profile_subscriptions (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    source_id BIGINT NOT NULL,
    upstream_key VARCHAR(512) NOT NULL,
    media_type VARCHAR(32) NOT NULL,
    local_profile_id INT,
    current_revision VARCHAR(255),
    base_document JSON NOT NULL,
    merged_document JSON NOT NULL,
    created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_profile_subscription (source_id, upstream_key, media_type),
    CONSTRAINT fk_profile_subscriptions_source FOREIGN KEY (source_id) REFERENCES profile_sources(id) ON DELETE CASCADE,
    CONSTRAINT fk_profile_subscriptions_profile FOREIGN KEY (local_profile_id) REFERENCES quality_profiles(id) ON DELETE SET NULL
) ENGINE=InnoDB;

CREATE TABLE profile_snapshots (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    subscription_id BIGINT NOT NULL,
    revision VARCHAR(255) NOT NULL,
    document JSON NOT NULL,
    changelog LONGTEXT,
    fetched_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_profile_snapshot (subscription_id, revision),
    CONSTRAINT fk_profile_snapshots_subscription FOREIGN KEY (subscription_id) REFERENCES profile_subscriptions(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE profile_overrides (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    subscription_id BIGINT NOT NULL,
    json_pointer VARCHAR(1024) NOT NULL,
    base_value JSON,
    local_value JSON,
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    UNIQUE KEY uq_profile_override (subscription_id, json_pointer(512)),
    CONSTRAINT fk_profile_overrides_subscription FOREIGN KEY (subscription_id) REFERENCES profile_subscriptions(id) ON DELETE CASCADE
) ENGINE=InnoDB;

CREATE TABLE custom_format_provenance (
    custom_format_id INT PRIMARY KEY,
    subscription_id BIGINT,
    upstream_key VARCHAR(512),
    upstream_revision VARCHAR(255),
    upstream_score INT,
    local_score INT,
    updated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    CONSTRAINT fk_custom_format_provenance_format FOREIGN KEY (custom_format_id) REFERENCES custom_formats(id) ON DELETE CASCADE,
    CONSTRAINT fk_custom_format_provenance_subscription FOREIGN KEY (subscription_id) REFERENCES profile_subscriptions(id) ON DELETE SET NULL
) ENGINE=InnoDB;

-- P6: replayable decision outcomes and ordered per-spec explanations.
CREATE TABLE decision_records (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    media_type VARCHAR(32) NOT NULL,
    media_id BIGINT,
    release_guid VARCHAR(768) NOT NULL,
    release_title VARCHAR(2048) NOT NULL,
    accepted BOOLEAN NOT NULL,
    total_score INT NOT NULL,
    input JSON NOT NULL,
    outcome JSON NOT NULL,
    evaluated_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
    KEY idx_decision_records_media (media_type, media_id, evaluated_at DESC),
    KEY idx_decision_records_guid (release_guid)
) ENGINE=InnoDB;

CREATE TABLE decision_steps (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    decision_id BIGINT NOT NULL,
    ordinal INT NOT NULL,
    specification VARCHAR(255) NOT NULL,
    accepted BOOLEAN NOT NULL,
    score_delta INT NOT NULL DEFAULT 0,
    reason LONGTEXT NOT NULL,
    details JSON,
    UNIQUE KEY uq_decision_step (decision_id, ordinal),
    CONSTRAINT fk_decision_steps_record FOREIGN KEY (decision_id) REFERENCES decision_records(id) ON DELETE CASCADE
) ENGINE=InnoDB;

INSERT INTO quality_profiles
    (name, cutoff, upgrade_allowed, min_format_score, cutoff_format_score, items)
VALUES
    ('Any', 20, TRUE, 0, 0, '[{"quality":1,"allowed":true},{"quality":2,"allowed":true},{"quality":3,"allowed":true},{"quality":4,"allowed":true},{"quality":5,"allowed":true},{"quality":6,"allowed":true},{"quality":7,"allowed":true},{"quality":8,"allowed":true},{"quality":9,"allowed":true},{"quality":10,"allowed":true},{"quality":11,"allowed":true},{"quality":12,"allowed":true},{"quality":13,"allowed":true},{"quality":14,"allowed":true},{"quality":15,"allowed":true},{"quality":16,"allowed":true},{"quality":17,"allowed":true},{"quality":18,"allowed":true},{"quality":19,"allowed":true}]'),
    ('HD-1080p', 13, TRUE, 0, 0, '[{"quality":10,"allowed":true},{"quality":11,"allowed":true},{"quality":12,"allowed":true},{"quality":13,"allowed":true},{"quality":14,"allowed":true}]'),
    ('Ultra-HD', 18, TRUE, 0, 0, '[{"quality":15,"allowed":true},{"quality":16,"allowed":true},{"quality":17,"allowed":true},{"quality":18,"allowed":true},{"quality":19,"allowed":true}]');

INSERT INTO naming_config
    (media_type, standard_format, daily_format, anime_format, season_folder_format, colon_replacement)
VALUES
    ('series', '{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]', '{Series Title} - {Air-Date} - {Episode Title} [{Quality Title}]', '{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]', 'Season {season:00}', 'smart');

INSERT INTO naming_config
    (media_type, movie_format, movie_folder_format, colon_replacement)
VALUES
    ('movie', '{Movie Title} ({Release Year}) [{Quality Title}]', '{Movie Title} ({Release Year})', 'smart');

INSERT INTO discover_sliders (slider_type, display_order, is_built_in, enabled, title)
VALUES
    ('trending', 1, TRUE, TRUE, 'Trending'),
    ('popular_movies', 2, TRUE, TRUE, 'Popular Movies'),
    ('popular_tv', 3, TRUE, TRUE, 'Popular TV Shows'),
    ('upcoming_movies', 4, TRUE, TRUE, 'Upcoming Movies'),
    ('upcoming_tv', 5, TRUE, TRUE, 'Upcoming TV Shows'),
    ('recently_added', 6, TRUE, TRUE, 'Recently Added'),
    ('movie_genres', 7, TRUE, TRUE, 'Movie Genres'),
    ('tv_genres', 8, TRUE, TRUE, 'TV Genres');

INSERT IGNORE INTO app_config (`key`, value)
VALUES ('recycle_bin_path', '""'), ('recycle_bin_cleanup_days', '7');

INSERT IGNORE INTO dav_items (id, id_prefix, name, item_type, sub_type, path)
VALUES
    ('00000000-0000-0000-0000-000000000001', '0000', 'dav', 1, 102, '/'),
    ('00000000-0000-0000-0000-000000000002', '0000', 'content', 1, 104, '/content'),
    ('00000000-0000-0000-0000-000000000003', '0000', 'nzbs', 1, 103, '/nzbs');
