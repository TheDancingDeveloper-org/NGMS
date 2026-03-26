-- Streaming session tracking
CREATE TABLE IF NOT EXISTS streaming_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    media_file_id BIGINT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    session_type TEXT NOT NULL,             -- 'direct' or 'transcode'
    status TEXT NOT NULL DEFAULT 'active',  -- 'active', 'paused', 'completed', 'error'
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_activity TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transcode_progress REAL,               -- 0.0 to 1.0
    video_codec TEXT,
    audio_codec TEXT,
    resolution TEXT,
    bitrate BIGINT,
    client_info TEXT,                       -- user-agent or similar
    transcode_dir TEXT                      -- path to temp HLS segments
);

CREATE INDEX IF NOT EXISTS idx_streaming_sessions_media ON streaming_sessions(media_file_id);
CREATE INDEX IF NOT EXISTS idx_streaming_sessions_status ON streaming_sessions(status);
