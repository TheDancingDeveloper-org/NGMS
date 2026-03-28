-- System-wide activity tracking (disk scans, imports, transcodes, etc.)
CREATE TABLE system_activities (
    id BIGSERIAL PRIMARY KEY,
    activity_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'running',
    title TEXT NOT NULL,
    detail TEXT,
    progress JSONB,
    result JSONB,
    error TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);
CREATE INDEX idx_system_activities_status ON system_activities(status, started_at DESC);
CREATE INDEX idx_system_activities_recent ON system_activities(started_at DESC);
