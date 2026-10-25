#!/bin/sh
# Hands the data saved before multi-user sign-in (owner = 'owner') to one named
# user, or takes it back. The user is never guessed: pass the id that the
# `users` table holds, which `status` prints after the user has signed in once.
#
#   scripts/legacy-owner.sh status|migrate|rollback USER_ID [--local|--remote]
#
# The D1 database is `app`. Take a backup first for --remote:
#   npx wrangler d1 export app --remote --output backup.sql
set -eu

action="${1:-}"
user="${2:-}"
target="${3:---local}"

case "$action" in status | migrate | rollback) ;; *) echo "usage: $0 status|migrate|rollback USER_ID [--local|--remote]" >&2; exit 2 ;; esac
case "$target" in --local | --remote) ;; *) echo "target must be --local or --remote" >&2; exit 2 ;; esac
case "$user" in
    '' | *[!A-Za-z0-9_-]*) echo "USER_ID must contain only letters, digits, - and _" >&2; exit 2 ;;
esac

dir=$(dirname "$0")/legacy-owner
# PERSIST_DIR points a --local run at a disposable local database.
wrangler="npx --yes wrangler@4.152.0"
persist=""
if [ -n "${PERSIST_DIR:-}" ]; then persist="--persist-to $PERSIST_DIR"; fi

found=$($wrangler d1 execute app "$target" $persist --json --command "SELECT count(*) AS n FROM users WHERE id='$user' AND status='active'" | sed -n 's/.*"n": *\([0-9]*\).*/\1/p' | head -n 1)
if [ "$found" != "1" ] && [ "$action" = "migrate" ]; then
    echo "no active user $user in the database: the user must sign in once first" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sed "s/@USER@/$user/g" "$dir/status.sql" > "$work/status.sql"
echo "== before"
$wrangler d1 execute app "$target" $persist --file "$work/status.sql"
if [ "$action" != "status" ]; then
    echo "== $action"
    sed "s/@USER@/$user/g" "$dir/$action.sql" > "$work/action.sql"
    $wrangler d1 execute app "$target" $persist --file "$work/action.sql"
    echo "== after"
    $wrangler d1 execute app "$target" $persist --file "$work/status.sql"
fi
