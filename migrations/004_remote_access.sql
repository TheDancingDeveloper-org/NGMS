-- Remote client authorization for bootstrap-discovered clients
CREATE TABLE IF NOT EXISTS remote_clients (
    id SERIAL PRIMARY KEY,
    client_token UUID NOT NULL UNIQUE,
    client_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ,
    revoked BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_remote_clients_token ON remote_clients(client_token);
