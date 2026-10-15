#!/usr/bin/env bash
# Back up Tideline's registry: entities, attributions and their evidence, clustering links,
# API keys and metering, disclosures, the token registry and the network binding. Flows
# and the ingest cursor are left out: the chain rebuilds them (set BACKFILL_BLOCKS on the
# restored service to cover the window you want back).
#
# Usage: scripts/backup-registry.sh <database-url> [output-dir]
#   The URL is Render's "External Database URL" (or any Postgres URL).
#   pg_dump must be at least the server's major version: PG_IMAGE=postgres:18 if needed.
#
# Restore into an empty database, then point the service's DATABASE_URL at it:
#   pg_restore --no-owner --no-privileges -d <new-database-url> <file>
set -euo pipefail

url="${1:?usage: $0 <database-url> [output-dir]}"
out="${2:-backups}"
mkdir -p "$out"
file="tideline-registry-$(date -u +%Y%m%dT%H%MZ).dump"
args=(-Fc --no-owner --no-privileges --exclude-table-data=flows --exclude-table-data=ingest_cursor)

if command -v pg_dump >/dev/null 2>&1 && [ -z "${PG_IMAGE:-}" ]; then
  pg_dump "$url" "${args[@]}" -f "$out/$file"
else
  runtime="$(command -v podman || command -v docker || true)"
  if [ -z "$runtime" ]; then
    echo "need pg_dump, podman or docker" >&2
    exit 1
  fi
  # Host networking, so a localhost URL works too; :Z for SELinux hosts.
  "$runtime" run --rm --network host -v "$(cd "$out" && pwd):/out:Z" \
    "${PG_IMAGE:-docker.io/library/postgres:17}" \
    pg_dump "$url" "${args[@]}" -f "/out/$file"
fi
echo "$out/$file ($(du -h "$out/$file" | cut -f1))"
