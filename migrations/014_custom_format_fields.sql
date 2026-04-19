ALTER TABLE custom_formats ADD COLUMN include_custom_format_when_renaming BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE quality_profiles ADD COLUMN min_upgrade_format_score INTEGER NOT NULL DEFAULT 1;
