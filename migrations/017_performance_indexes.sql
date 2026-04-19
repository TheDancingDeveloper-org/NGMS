-- Performance indexes for stream path resolution and file lookups

CREATE INDEX IF NOT EXISTS idx_episode_files_media_file ON episode_files(media_file_id);
CREATE INDEX IF NOT EXISTS idx_episode_files_episode ON episode_files(episode_id);
CREATE INDEX IF NOT EXISTS idx_movies_movie_file ON movies(movie_file_id);
CREATE INDEX IF NOT EXISTS idx_media_files_media_type ON media_files(media_type);
