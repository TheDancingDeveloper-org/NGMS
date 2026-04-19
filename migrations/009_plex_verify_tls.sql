-- Add per-server TLS verification toggle for Plex connections.
-- Default false for backward compat (existing servers commonly use self-signed certs).
ALTER TABLE plex_servers ADD COLUMN verify_tls BOOLEAN NOT NULL DEFAULT false;
