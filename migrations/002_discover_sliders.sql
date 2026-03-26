-- Discover sliders for configurable homepage discovery carousels

CREATE TABLE discover_sliders (
    id SERIAL PRIMARY KEY,
    slider_type TEXT NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 0,
    is_built_in BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true,
    title TEXT,
    custom_data JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_discover_sliders_order ON discover_sliders(display_order);

-- Seed default built-in sliders
INSERT INTO discover_sliders (slider_type, display_order, is_built_in, enabled, title) VALUES
('trending',           1,  true, true, 'Trending'),
('popular_movies',     2,  true, true, 'Popular Movies'),
('popular_tv',         3,  true, true, 'Popular TV Shows'),
('upcoming_movies',    4,  true, true, 'Upcoming Movies'),
('upcoming_tv',        5,  true, true, 'Upcoming TV Shows'),
('recently_added',     6,  true, true, 'Recently Added'),
('movie_genres',       7,  true, true, 'Movie Genres'),
('tv_genres',          8,  true, true, 'TV Genres');
