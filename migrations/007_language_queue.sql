-- Add language preference to quality profiles (Radarr v6 compatibility).
-- -1 = any language (default), -2 = original language, positive = specific Radarr language ID.
ALTER TABLE quality_profiles ADD COLUMN language INTEGER NOT NULL DEFAULT -1;

-- Add original language to movies for resolving "Original" language profiles.
-- Stores Radarr language ID (1=English, 2=French, 3=Spanish, etc.).
ALTER TABLE movies ADD COLUMN original_language INTEGER;

-- Add index on queue for media-item-based conflict checking (replaces guid-only lookups).
CREATE INDEX idx_queue_media ON queue(media_type, media_id);
