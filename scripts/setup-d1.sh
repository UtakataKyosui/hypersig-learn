#!/bin/sh
# Stores the id of the D1 database as the GitHub secret D1_DATABASE_ID.
#
#   scripts/setup-d1.sh [--dry-run]
#
# Creates the database named by `database_name` in wrangler.toml when the
# account has none, then reads its id. The deploy workflow writes the id into
# wrangler.toml before it deploys. --dry-run prints the id and touches nothing
# on GitHub; it still creates the database when it is missing.
set -eu

cd "$(dirname "$0")/.."

dry_run=false
case "${1:-}" in
    --dry-run) dry_run=true ;;
    "") ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
esac

name=$(sed -En 's/^database_name = "([^"]+)".*/\1/p' wrangler.toml | head -n 1)
if [ -z "$name" ]; then
    echo "no database_name in wrangler.toml" >&2
    exit 1
fi

if ! npx wrangler d1 info "$name" --json > /dev/null 2>&1; then
    npx wrangler d1 create "$name" > /dev/null
fi
id=$(npx wrangler d1 info "$name" --json | sed -En 's/^ *"uuid": *"([0-9a-f-]{36})".*/\1/p' | head -n 1)
if [ -z "$id" ]; then
    echo "could not read the id of database $name" >&2
    exit 1
fi

if [ "$dry_run" = true ]; then
    printf '%s %s\n' "$name" "$id"
else
    printf '%s' "$id" | gh secret set D1_DATABASE_ID > /dev/null
    echo "D1_DATABASE_ID stored for database $name."
fi
