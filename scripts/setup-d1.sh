#!/bin/sh
# Stores the id of the D1 database as the GitHub secret D1_DATABASE_ID.
#
#   scripts/setup-d1.sh [--dry-run]
#
# Creates the database named by cloudflare.config.ts when the account has
# none, then reads its id. CI passes that ID through D1_DATABASE_ID.
# --dry-run prints the id and touches nothing
# on GitHub; it still creates the database when it is missing.
set -eu

cd "$(dirname "$0")/.."

dry_run=false
case "${1:-}" in
    --dry-run) dry_run=true ;;
    "") ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
esac

name=$(node --input-type=module -e 'import {databaseName} from "./cloudflare.config.ts"; console.log(databaseName)')
# A failed lookup must stop, rather than be mistaken for a missing database.
databases=$(npx --no-install cf d1 list --name "$name" --per-page 1000)
id=$(printf '%s' "$databases" | python3 -c '
import json, sys
matches = [db for db in json.load(sys.stdin) if db["name"] == sys.argv[1]]
if len(matches) > 1:
    sys.exit("more than one matching D1 database")
print(matches[0]["uuid"] if matches else "")
' "$name")
if [ -z "$id" ]; then
    database=$(npx --no-install cf d1 create --name "$name")
    id=$(printf '%s' "$database" | python3 -c 'import json, sys; print(json.load(sys.stdin)["uuid"])')
fi
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
