#!/bin/sh
# Hands the data saved before multi-user sign-in (owner = 'owner') to one named
# user, or takes it back. The user is never guessed: pass the id that the
# `users` table holds. `users` lists them with their sign-in time, status and
# the number of medications each owns, to choose from after the person has
# signed in once. The id is not shown in the app.
#
#   scripts/legacy-owner.sh users [--local|--remote]
#   scripts/legacy-owner.sh status|migrate|rollback USER_ID [--local|--remote]
#
# The D1 database is `app`. Take a backup first for --remote:
#   npx wrangler d1 export app --remote --output backup.sql
set -eu

action="${1:-}"
if [ "$action" = users ]; then
    user=""
    target="${2:---local}"
else
    user="${2:-}"
    target="${3:---local}"
fi

case "$action" in users | status | migrate | rollback) ;; *) echo "usage: $0 users [--local|--remote] | status|migrate|rollback USER_ID [--local|--remote]" >&2; exit 2 ;; esac
case "$target" in --local | --remote) ;; *) echo "target must be --local or --remote" >&2; exit 2 ;; esac
case "$user" in
    '') [ "$action" = users ] || { echo "USER_ID is required" >&2; exit 2; } ;;
    *[!A-Za-z0-9_-]*) echo "USER_ID must contain only letters, digits, - and _" >&2; exit 2 ;;
esac

dir=$(dirname "$0")/legacy-owner
# PERSIST_DIR points a --local run at a disposable local database.
wrangler="npx --yes wrangler@4.152.0"
persist=""
if [ -n "${PERSIST_DIR:-}" ]; then persist="--persist-to $PERSIST_DIR"; fi

if [ "$action" = users ]; then
    $wrangler d1 execute app "$target" $persist --command "SELECT u.id, datetime(u.created_at/1000,'unixepoch') AS created_utc, u.status, (SELECT count(*) FROM medications m WHERE m.owner=u.id) AS medications FROM users u ORDER BY u.created_at"
    exit 0
fi

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
