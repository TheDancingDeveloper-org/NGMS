-- Discovered media files/groups that the disk scanner could not match to an
-- existing series/movie in the DB. The scanner writes candidate rows here
-- instead of dropping unmatched files silently, so the user can review,
-- accept, or reject them via the Import UI.
--
-- A single candidate can cover a whole series (many files grouped by parsed
-- title+year) or a single movie file, depending on the scanner's confidence
-- that the grouping is correct.
CREATE TABLE import_candidates (
    id                 BIGSERIAL PRIMARY KEY,
    media_library_folder_id INTEGER REFERENCES media_library_folders(id) ON DELETE CASCADE,
    media_type         TEXT        NOT NULL,            -- 'series' | 'movie'
    -- Kind of grouping this row represents.  'series' = entire show folder,
    -- 'season' = one season of a show, 'episode' = a single episode file,
    -- 'movie' = a single movie file.
    match_kind         TEXT        NOT NULL,
    -- Path the candidate was derived from. For series/season kinds this is
    -- the folder; for episode/movie kinds this is the file.
    discovered_path    TEXT        NOT NULL,
    file_count         INTEGER     NOT NULL DEFAULT 1,
    total_size         BIGINT      NOT NULL DEFAULT 0,

    -- Parsed metadata (from stackarr-parser on filenames / folder names).
    parsed_title       TEXT,
    parsed_year        INTEGER,
    parsed_season      INTEGER,
    parsed_episodes    INTEGER[],

    -- TMDB suggestion (populated by the match pass).
    suggested_tmdb_id  INTEGER,
    suggested_title    TEXT,
    suggested_year     INTEGER,
    suggested_poster   TEXT,
    suggested_overview TEXT,
    confidence         REAL        NOT NULL DEFAULT 0.0,

    -- Review state.
    -- 'pending'  — awaiting user review
    -- 'accepted' — user accepted; resulting series_id/movie_id set on target_*
    -- 'rejected' — user explicitly rejected
    -- 'ignored'  — skipped (e.g. bulk-ignore low confidence)
    -- 'failed'   — accept action raised an error (see `error`)
    status             TEXT        NOT NULL DEFAULT 'pending',
    target_series_id   BIGINT      REFERENCES series(id)  ON DELETE SET NULL,
    target_movie_id    BIGINT      REFERENCES movies(id)  ON DELETE SET NULL,
    error              TEXT,

    -- Raw parsed metadata + per-file breakdown for the UI to display.
    data               JSONB       NOT NULL DEFAULT '{}'::jsonb,

    discovered_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at        TIMESTAMPTZ
);

CREATE INDEX idx_import_candidates_status ON import_candidates(status);
CREATE INDEX idx_import_candidates_media_type ON import_candidates(media_type);
CREATE INDEX idx_import_candidates_discovered_path ON import_candidates(discovered_path);

-- Prevent duplicate pending rows for the same discovered path.  When the
-- scheduler re-runs disk_scan every 12h we only want a new row per path
-- when the previous one has been resolved (accepted/rejected/ignored/failed).
CREATE UNIQUE INDEX idx_import_candidates_pending_path
    ON import_candidates(discovered_path)
    WHERE status = 'pending';
