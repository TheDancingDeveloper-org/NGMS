-- Track files moved to the recycle bin for scheduled cleanup
CREATE TABLE recycle_bin (
    id         BIGSERIAL PRIMARY KEY,
    original_path TEXT      NOT NULL,
    recycle_path  TEXT      NOT NULL,
    media_file_id BIGINT,
    media_type    TEXT      NOT NULL,
    media_id      BIGINT   NOT NULL,
    size          BIGINT   NOT NULL DEFAULT 0,
    recycled_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_recycle_bin_recycled_at ON recycle_bin(recycled_at);

-- Seed default media management config
INSERT INTO app_config (key, value) VALUES
    ('recycle_bin_path', '""'::jsonb),
    ('recycle_bin_cleanup_days', '7'::jsonb)
ON CONFLICT (key) DO NOTHING;
