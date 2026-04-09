-- NZBDav virtual filesystem for live Usenet streaming
-- Module: dav_streaming

-- Virtual filesystem nodes
CREATE TABLE dav_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    id_prefix TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    parent_id UUID REFERENCES dav_items(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_size BIGINT,
    item_type INT NOT NULL,          -- 1=Directory, 2=UsenetFile
    sub_type INT NOT NULL,           -- 101-106=dir types, 201-204=file types
    path TEXT NOT NULL,
    release_date TIMESTAMPTZ,
    last_health_check TIMESTAMPTZ,
    next_health_check TIMESTAMPTZ,
    history_item_id UUID,
    file_blob_id UUID,
    nzb_blob_id UUID,
    UNIQUE(parent_id, name)
);

CREATE INDEX idx_dav_items_prefix ON dav_items(id_prefix, item_type);
CREATE INDEX idx_dav_items_type_created ON dav_items(item_type, created_at);
CREATE INDEX idx_dav_items_sub_type ON dav_items(sub_type, created_at);
CREATE INDEX idx_dav_items_history ON dav_items(history_item_id, item_type);
CREATE INDEX idx_dav_items_nzb_blob ON dav_items(nzb_blob_id);
CREATE INDEX idx_dav_items_path ON dav_items(path);

-- File metadata blobs (DavMultipartFile / DavNzbFile serialized as bincode)
CREATE TABLE dav_blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    data BYTEA NOT NULL
);

-- Raw NZB XML blobs
CREATE TABLE dav_nzb_blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    data BYTEA NOT NULL
);

-- NZB processing queue (for batch operations)
CREATE TABLE dav_queue_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    file_name TEXT NOT NULL,
    job_name TEXT NOT NULL,
    nzb_file_size BIGINT NOT NULL DEFAULT 0,
    total_segment_bytes BIGINT NOT NULL DEFAULT 0,
    category TEXT NOT NULL DEFAULT '',
    priority INT NOT NULL DEFAULT 0,
    post_processing INT NOT NULL DEFAULT -1,
    pause_until TIMESTAMPTZ
);

CREATE INDEX idx_dav_queue_priority ON dav_queue_items(priority DESC, created_at ASC);

-- Processing history
CREATE TABLE dav_history_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    file_name TEXT NOT NULL,
    job_name TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    download_status INT NOT NULL,    -- 1=Completed, 2=Failed
    total_segment_bytes BIGINT NOT NULL DEFAULT 0,
    download_time_seconds INT NOT NULL DEFAULT 0,
    fail_message TEXT,
    download_dir_id UUID,
    nzb_blob_id UUID
);

CREATE INDEX idx_dav_history_created ON dav_history_items(created_at);

-- Health checks
CREATE TABLE dav_health_checks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dav_item_id UUID NOT NULL REFERENCES dav_items(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    result INT NOT NULL DEFAULT 0,
    repair_status INT NOT NULL DEFAULT 0,
    message TEXT NOT NULL DEFAULT ''
);

-- Module-specific config (separate from app_config)
CREATE TABLE dav_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
