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
# Set D1_DATABASE_ID for --remote and take a Cloudflare dashboard backup first.
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

cd "$(dirname "$0")/.."
dir=scripts/legacy-owner
# PERSIST_DIR points a --local run at a disposable local database.
query() {
    if [ "$target" = --local ]; then
        npx --no-install cf d1 raw 00000000-0000-4000-8000-000000000001 --local --persist-to "${PERSIST_DIR:-.wrangler/state}" --sql "$1"
    else
        : "${D1_DATABASE_ID:?Set D1_DATABASE_ID for --remote}"
        npx --no-install cf d1 raw "$D1_DATABASE_ID" --sql "$1"
    fi
}

if [ "$action" = users ]; then
    query "SELECT u.id, datetime(u.created_at/1000,'unixepoch') AS created_utc, u.status, (SELECT count(*) FROM medications m WHERE m.owner=u.id) AS medications FROM users u ORDER BY u.created_at"
    exit 0
fi

result=$(query "SELECT count(*) AS n FROM users WHERE id='$user' AND status='active'")
found=$(printf '%s' "$result" | python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["results"]["rows"][0][0])')
if [ "$found" != "1" ] && [ "$action" = "migrate" ]; then
    echo "no active user $user in the database: the user must sign in once first" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sed "s/@USER@/$user/g" "$dir/status.sql" > "$work/status.sql"
echo "== before"
query "@$work/status.sql"
if [ "$action" != "status" ]; then
    echo "== $action"
    sed "s/@USER@/$user/g" "$dir/$action.sql" > "$work/action.sql"
    query "@$work/action.sql"
    echo "== after"
    query "@$work/status.sql"
fi
