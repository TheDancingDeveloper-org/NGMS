-- Add media_type to quality profiles (series, movie, or NULL for any/both)
ALTER TABLE quality_profiles ADD COLUMN media_type TEXT;
