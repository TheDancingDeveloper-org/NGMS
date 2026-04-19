#!/bin/bash
# Create the indexarr database if it doesn't exist.
# Postgres entrypoint runs this on first init only.
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<-EOSQL
    SELECT 'CREATE DATABASE indexarr'
    WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = 'indexarr')\gexec
EOSQL
