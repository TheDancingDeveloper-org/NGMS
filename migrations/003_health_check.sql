-- Health check tracking for download clients
ALTER TABLE download_clients
  ADD COLUMN last_health_check TIMESTAMPTZ,
  ADD COLUMN health_status TEXT NOT NULL DEFAULT 'unknown',
  ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN auto_disabled BOOLEAN NOT NULL DEFAULT false;

-- Health check tracking for indexers
ALTER TABLE indexers
  ADD COLUMN last_health_check TIMESTAMPTZ,
  ADD COLUMN health_status TEXT NOT NULL DEFAULT 'unknown',
  ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN auto_disabled BOOLEAN NOT NULL DEFAULT false;
