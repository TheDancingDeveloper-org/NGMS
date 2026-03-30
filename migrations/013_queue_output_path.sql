-- Add output_path to store the resolved download output directory from the client.
-- Add stale_count to track how many consecutive polling cycles the item is missing from the client.
ALTER TABLE queue ADD COLUMN output_path TEXT;
ALTER TABLE queue ADD COLUMN stale_count INTEGER NOT NULL DEFAULT 0;
